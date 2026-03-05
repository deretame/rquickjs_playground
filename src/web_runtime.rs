use serde_json::Map;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::Method;
use reqwest::blocking::Client;

use filetime::{FileTime, set_file_times};
use rquickjs::{Ctx, Promise, function::Func};
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::WasiCtxBuilder;

#[cfg(test)]
use rquickjs::{Context, Runtime};

pub const WEB_POLYFILL: &str = concat!(
    include_str!("../js/00_bootstrap.js"),
    "\n",
    include_str!("../js/10_headers.js"),
    "\n",
    include_str!("../js/20_abort.js"),
    "\n",
    include_str!("../js/30_fetch.js"),
    "\n",
    include_str!("../js/40_xhr.js"),
    "\n",
    include_str!("../js/50_fs.js"),
    "\n",
    include_str!("../js/60_native.js"),
    "\n",
    include_str!("../js/61_wasi.js"),
    "\n",
    include_str!("../js/62_bridge.js"),
    "\n",
    include_str!("../js/63_plugin.js"),
    "\n",
    include_str!("../js/99_exports.js"),
    "\n"
);

pub const AXIOS_BUNDLE: &str = include_str!("../vendor/axios.min.js");

pub fn install_host_bindings(ctx: &Ctx<'_>) -> Result<(), rquickjs::Error> {
    let globals = ctx.globals();
    globals.set("__http_request", Func::from(http_request))?;
    globals.set("__native_buffer_put", Func::from(native_buffer_put))?;
    globals.set("__native_buffer_put_raw", Func::from(native_buffer_put_raw))?;
    globals.set("__native_buffer_take", Func::from(native_buffer_take))?;
    globals.set(
        "__native_buffer_take_raw",
        Func::from(native_buffer_take_raw),
    )?;
    globals.set("__native_buffer_free", Func::from(native_buffer_free))?;
    globals.set("__native_exec", Func::from(native_exec))?;
    globals.set("__native_exec_chain", Func::from(native_exec_chain))?;
    globals.set("__host_call", Func::from(host_call))?;
    globals.set("__wasi_run", Func::from(wasi_run))?;
    globals.set("__fs_read_file", Func::from(fs_read_file))?;
    globals.set("__fs_write_file", Func::from(fs_write_file))?;
    globals.set("__fs_mkdir", Func::from(fs_mkdir))?;
    globals.set("__fs_readdir", Func::from(fs_readdir))?;
    globals.set("__fs_stat", Func::from(fs_stat))?;
    globals.set("__fs_access", Func::from(fs_access))?;
    globals.set("__fs_unlink", Func::from(fs_unlink))?;
    globals.set("__fs_rm", Func::from(fs_rm))?;
    globals.set("__fs_rename", Func::from(fs_rename))?;
    globals.set("__fs_copy_file", Func::from(fs_copy_file))?;
    globals.set("__fs_realpath", Func::from(fs_realpath))?;
    globals.set("__fs_lstat", Func::from(fs_lstat))?;
    globals.set("__fs_readlink", Func::from(fs_readlink))?;
    globals.set("__fs_symlink", Func::from(fs_symlink))?;
    globals.set("__fs_link", Func::from(fs_link))?;
    globals.set("__fs_truncate", Func::from(fs_truncate))?;
    globals.set("__fs_chmod", Func::from(fs_chmod))?;
    globals.set("__fs_utimes", Func::from(fs_utimes))?;
    globals.set("__fs_cp", Func::from(fs_cp))?;
    globals.set("__fs_mkdtemp", Func::from(fs_mkdtemp))?;
    Ok(())
}

pub fn http_request(
    method: String,
    url: String,
    headers_json: String,
    body: Option<String>,
) -> String {
    match http_request_inner(method, url, headers_json, body) {
        Ok(payload) => payload,
        Err(error) => json!({ "ok": false, "error": error }).to_string(),
    }
}

fn http_request_inner(
    method: String,
    url: String,
    headers_json: String,
    body: Option<String>,
) -> Result<String, String> {
    let method = Method::from_bytes(method.as_bytes()).map_err(|e| e.to_string())?;
    let mut headers_map = Map::new();
    let headers_value: Value = serde_json::from_str(&headers_json).map_err(|e| e.to_string())?;
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let mut builder = client.request(method, &url);

    if let Value::Object(obj) = headers_value {
        for (key, value) in obj {
            if let Some(v) = value.as_str() {
                builder = builder.header(&key, v);
            }
        }
    }

    if let Some(content) = body {
        builder = builder.body(content);
    }

    let response = builder.send().map_err(|e| e.to_string())?;
    let status = response.status();
    let final_url = response.url().to_string();

    for (name, value) in response.headers() {
        let value_text = value.to_str().map_err(|e| e.to_string())?.to_string();
        headers_map.insert(name.to_string(), Value::String(value_text));
    }

    let body_text = response.text().map_err(|e| e.to_string())?;

    Ok(json!({
        "ok": true,
        "status": status.as_u16(),
        "statusText": status.canonical_reason().unwrap_or(""),
        "url": final_url,
        "headers": headers_map,
        "body": body_text
    })
    .to_string())
}

static NATIVE_BUF_ID: AtomicU64 = AtomicU64::new(1);
static NATIVE_BUF_POOL: OnceLock<Mutex<HashMap<u64, Vec<u8>>>> = OnceLock::new();

fn native_pool() -> &'static Mutex<HashMap<u64, Vec<u8>>> {
    NATIVE_BUF_POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn parse_u8_json_array(data_json: &str) -> Result<Vec<u8>, String> {
    let value: Value = serde_json::from_str(data_json).map_err(|e| e.to_string())?;
    let arr = value
        .as_array()
        .ok_or_else(|| "数据必须是字节数组".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let n = item
            .as_u64()
            .ok_or_else(|| "字节数组元素必须是整数".to_string())?;
        if n > 255 {
            return Err("字节数组元素必须在 0-255 范围".to_string());
        }
        out.push(n as u8);
    }
    Ok(out)
}

pub fn native_buffer_put(data_json: String) -> String {
    let bytes = match parse_u8_json_array(&data_json) {
        Ok(bytes) => bytes,
        Err(error) => return json!({ "ok": false, "error": error }).to_string(),
    };
    let id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    pool.insert(id, bytes);
    json!({ "ok": true, "id": id }).to_string()
}

pub fn native_buffer_put_raw(bytes: Vec<u8>) -> u64 {
    let id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    pool.insert(id, bytes);
    id
}

pub fn native_buffer_take(id: u64) -> String {
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    match pool.remove(&id) {
        Some(bytes) => json!({ "ok": true, "data": bytes }).to_string(),
        None => json!({ "ok": false, "error": "buffer id 不存在" }).to_string(),
    }
}

pub fn native_buffer_take_raw(id: u64) -> Option<Vec<u8>> {
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    pool.remove(&id)
}

pub fn native_buffer_free(id: u64) -> String {
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    let existed = pool.remove(&id).is_some();
    json!({ "ok": true, "freed": existed }).to_string()
}

fn native_apply_op(
    op: &str,
    mut bytes: Vec<u8>,
    extra: Option<Vec<u8>>,
) -> Result<Vec<u8>, String> {
    match op {
        "invert" => {
            for b in &mut bytes {
                *b = 255 - *b;
            }
            Ok(bytes)
        }
        "grayscale_rgba" => {
            for chunk in bytes.chunks_exact_mut(4) {
                let r = chunk[0] as f32;
                let g = chunk[1] as f32;
                let b = chunk[2] as f32;
                let y = (0.299 * r + 0.587 * g + 0.114 * b).round() as u8;
                chunk[0] = y;
                chunk[1] = y;
                chunk[2] = y;
            }
            Ok(bytes)
        }
        "xor" => {
            let rhs = extra.ok_or_else(|| "xor 需要第二个输入参数".to_string())?;
            if rhs.len() != bytes.len() {
                return Err("xor 两个输入长度必须一致".to_string());
            }
            for i in 0..bytes.len() {
                bytes[i] ^= rhs[i];
            }
            Ok(bytes)
        }
        "noop" => Ok(bytes),
        _ => Err(format!("不支持的 native op: {op}")),
    }
}

fn parse_chain_steps(steps_json: &str) -> Result<Vec<(String, Option<u64>)>, String> {
    let value: Value = serde_json::from_str(steps_json).map_err(|e| e.to_string())?;
    let arr = value
        .as_array()
        .ok_or_else(|| "steps 必须是数组".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let obj = item
            .as_object()
            .ok_or_else(|| "steps 元素必须是对象".to_string())?;
        let op = obj
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| "steps 元素缺少 op 字段".to_string())?
            .to_string();
        let extra_input_id = obj.get("extraInputId").and_then(Value::as_u64);
        out.push((op, extra_input_id));
    }
    Ok(out)
}

pub fn native_exec(
    op: String,
    input_id: u64,
    _args_json: Option<String>,
    extra_input_id: Option<u64>,
) -> String {
    let (input, extra) = {
        let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
        let input = match pool.remove(&input_id) {
            Some(bytes) => bytes,
            None => return json!({ "ok": false, "error": "input id 不存在" }).to_string(),
        };

        let extra = if let Some(extra_id) = extra_input_id {
            match pool.remove(&extra_id) {
                Some(bytes) => Some(bytes),
                None => {
                    pool.insert(input_id, input);
                    return json!({ "ok": false, "error": "extra input id 不存在" }).to_string();
                }
            }
        } else {
            None
        };

        (input, extra)
    };

    let output = match native_apply_op(&op, input, extra) {
        Ok(bytes) => bytes,
        Err(error) => return json!({ "ok": false, "error": error }).to_string(),
    };

    let output_id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    pool.insert(output_id, output);
    json!({ "ok": true, "id": output_id }).to_string()
}

pub fn native_exec_chain(input_id: u64, steps_json: String) -> String {
    let steps = match parse_chain_steps(&steps_json) {
        Ok(steps) => steps,
        Err(error) => return json!({ "ok": false, "error": error }).to_string(),
    };
    if steps.is_empty() {
        return json!({ "ok": false, "error": "steps 不能为空" }).to_string();
    }

    let mut current = {
        let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
        match pool.remove(&input_id) {
            Some(bytes) => bytes,
            None => return json!({ "ok": false, "error": "input id 不存在" }).to_string(),
        }
    };

    for (op, extra_input_id) in steps {
        let extra = if let Some(extra_id) = extra_input_id {
            let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
            match pool.remove(&extra_id) {
                Some(bytes) => Some(bytes),
                None => {
                    return json!({ "ok": false, "error": "extra input id 不存在" }).to_string();
                }
            }
        } else {
            None
        };

        current = match native_apply_op(&op, current, extra) {
            Ok(bytes) => bytes,
            Err(error) => return json!({ "ok": false, "error": error }).to_string(),
        };
    }

    let output_id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    pool.insert(output_id, current);
    json!({ "ok": true, "id": output_id }).to_string()
}

fn parse_argv(args_json: Option<String>) -> Result<Vec<String>, String> {
    let Some(raw) = args_json else {
        return Ok(Vec::new());
    };
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let arr = value
        .as_array()
        .ok_or_else(|| "argv 必须是字符串数组".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(
            item.as_str()
                .ok_or_else(|| "argv 必须是字符串数组".to_string())?
                .to_string(),
        );
    }
    Ok(out)
}

pub fn wasi_run(
    module_id: u64,
    stdin_id: Option<u64>,
    args_json: Option<String>,
    consume_module: bool,
) -> String {
    let wasm_bytes = {
        let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
        if consume_module {
            match pool.remove(&module_id) {
                Some(bytes) => bytes,
                None => {
                    return json!({ "ok": false, "error": "module id 不存在" }).to_string();
                }
            }
        } else {
            match pool.get(&module_id) {
                Some(bytes) => bytes.clone(),
                None => {
                    return json!({ "ok": false, "error": "module id 不存在" }).to_string();
                }
            }
        }
    };

    let stdin_bytes = if let Some(id) = stdin_id {
        let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
        match pool.remove(&id) {
            Some(bytes) => bytes,
            None => return json!({ "ok": false, "error": "stdin id 不存在" }).to_string(),
        }
    } else {
        Vec::new()
    };

    let args = match parse_argv(args_json) {
        Ok(v) => v,
        Err(error) => return json!({ "ok": false, "error": error }).to_string(),
    };

    let engine = Engine::default();
    let module = match Module::new(&engine, &wasm_bytes) {
        Ok(module) => module,
        Err(error) => return json!({ "ok": false, "error": error.to_string() }).to_string(),
    };

    let mut linker: Linker<wasmtime_wasi::p1::WasiP1Ctx> = Linker::new(&engine);
    if let Err(error) = wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |s| s) {
        return json!({ "ok": false, "error": error.to_string() }).to_string();
    }

    let stdout_pipe = wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(1024 * 1024 * 64);
    let stderr_pipe = wasmtime_wasi::p2::pipe::MemoryOutputPipe::new(1024 * 1024 * 64);
    let stdin_pipe = wasmtime_wasi::p2::pipe::MemoryInputPipe::new(stdin_bytes);

    let mut builder = WasiCtxBuilder::new();
    builder.stdin(stdin_pipe);
    builder.stdout(stdout_pipe.clone());
    builder.stderr(stderr_pipe.clone());

    let mut argv = vec!["module.wasm".to_string()];
    argv.extend(args);
    builder.args(&argv);

    let wasi = builder.build_p1();
    let mut store = Store::new(&engine, wasi);

    let instance = match linker.instantiate(&mut store, &module) {
        Ok(instance) => instance,
        Err(error) => return json!({ "ok": false, "error": error.to_string() }).to_string(),
    };

    let start = match instance.get_typed_func::<(), ()>(&mut store, "_start") {
        Ok(func) => func,
        Err(error) => return json!({ "ok": false, "error": error.to_string() }).to_string(),
    };

    let mut exit_code = 0_i32;
    if let Err(error) = start.call(&mut store, ()) {
        if let Some(code) = error.downcast_ref::<wasmtime_wasi::I32Exit>() {
            exit_code = code.0;
        } else {
            return json!({ "ok": false, "error": error.to_string() }).to_string();
        }
    }

    let stdout_bytes = stdout_pipe.contents().to_vec();
    let stderr_bytes = stderr_pipe.contents().to_vec();

    let stdout_id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
    let stderr_id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    pool.insert(stdout_id, stdout_bytes);
    pool.insert(stderr_id, stderr_bytes);

    json!({
        "ok": true,
        "exitCode": exit_code,
        "stdoutId": stdout_id,
        "stderrId": stderr_id
    })
    .to_string()
}

fn parse_host_ok_payload(raw: String) -> Result<Value, String> {
    let payload: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if payload.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(payload)
    } else {
        Err(payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("调用失败")
            .to_string())
    }
}

fn parse_bridge_args(args_json: Option<String>) -> Result<Vec<Value>, String> {
    let Some(raw) = args_json else {
        return Ok(Vec::new());
    };
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| "args 必须是数组".to_string())
}

pub fn call_js_global_function(
    ctx: &Ctx<'_>,
    function_name: String,
    args_json: Option<String>,
) -> Result<Value, String> {
    let args = parse_bridge_args(args_json)?;
    let function_name_json = serde_json::to_string(&function_name).map_err(|e| e.to_string())?;
    let args_literal = serde_json::to_string(&args).map_err(|e| e.to_string())?;

    let script = format!(
        r#"
        (async () => {{
          const fnName = {function_name_json};
          const args = {args_literal};
          const fn = globalThis[fnName];
          if (typeof fn !== "function") {{
            throw new Error(`JS 函数不存在: ${{fnName}}`);
          }}
          const data = await fn(...args);
          return JSON.stringify({{ ok: true, data }});
        }})()
        "#
    );

    let promise: Promise = ctx.eval(script).map_err(|e| e.to_string())?;
    let raw: String = promise.finish().map_err(|e| e.to_string())?;
    let payload: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    if payload.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(payload.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(payload
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("调用失败")
            .to_string())
    }
}

pub fn plugin_load(ctx: &Ctx<'_>, script: String) -> Result<(), String> {
    ctx.eval::<(), _>(script).map_err(|e| e.to_string())
}

pub fn plugin_get_info(ctx: &Ctx<'_>, name: String) -> Result<Value, String> {
    call_js_global_function(
        ctx,
        "__plugin_host_get_info".to_string(),
        Some(json!([name]).to_string()),
    )
}

pub fn plugin_list(ctx: &Ctx<'_>) -> Result<Value, String> {
    call_js_global_function(ctx, "__plugin_host_list".to_string(), None)
}

fn require_arg<'a>(args: &'a [Value], index: usize, name: &str) -> Result<&'a Value, String> {
    args.get(index).ok_or_else(|| format!("缺少参数: {name}"))
}

fn require_str_arg(args: &[Value], index: usize, name: &str) -> Result<String, String> {
    require_arg(args, index, name)?
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| format!("参数 {name} 必须是字符串"))
}

fn require_u64_arg(args: &[Value], index: usize, name: &str) -> Result<u64, String> {
    require_arg(args, index, name)?
        .as_u64()
        .ok_or_else(|| format!("参数 {name} 必须是非负整数"))
}

fn parse_u8_json_value(value: &Value) -> Result<Vec<u8>, String> {
    let arr = value
        .as_array()
        .ok_or_else(|| "数据必须是字节数组".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let n = item
            .as_u64()
            .ok_or_else(|| "字节数组元素必须是整数".to_string())?;
        if n > 255 {
            return Err("字节数组元素必须在 0-255 范围".to_string());
        }
        out.push(n as u8);
    }
    Ok(out)
}

fn bridge_call_inner(name: String, args_json: Option<String>) -> Result<Value, String> {
    let args = parse_bridge_args(args_json)?;

    match name.as_str() {
        "math.add" => {
            let a = require_arg(&args, 0, "a")?
                .as_f64()
                .ok_or_else(|| "参数 a 必须是数字".to_string())?;
            let b = require_arg(&args, 1, "b")?
                .as_f64()
                .ok_or_else(|| "参数 b 必须是数字".to_string())?;
            Ok(json!(a + b))
        }
        "native.put" => {
            let bytes = parse_u8_json_value(require_arg(&args, 0, "bytes")?)?;
            let id = native_buffer_put_raw(bytes);
            Ok(json!(id))
        }
        "native.take" => {
            let id = require_u64_arg(&args, 0, "id")?;
            match native_buffer_take_raw(id) {
                Some(bytes) => Ok(json!(bytes)),
                None => Err("buffer id 不存在".to_string()),
            }
        }
        "native.exec" => {
            let op = require_str_arg(&args, 0, "op")?;
            let input_id = require_u64_arg(&args, 1, "inputId")?;
            let args_json = args.get(2).and_then(|v| {
                if v.is_null() {
                    None
                } else {
                    Some(v.to_string())
                }
            });
            let extra_input_id = args.get(3).and_then(Value::as_u64);
            let payload =
                parse_host_ok_payload(native_exec(op, input_id, args_json, extra_input_id))?;
            Ok(payload.get("id").cloned().unwrap_or(Value::Null))
        }
        _ => Err(format!("不支持的 bridge 方法: {name}")),
    }
}

pub fn host_call(name: String, args_json: Option<String>) -> String {
    match bridge_call_inner(name, args_json) {
        Ok(data) => json!({ "ok": true, "data": data }).to_string(),
        Err(error) => json!({ "ok": false, "error": error }).to_string(),
    }
}

fn io_error_code(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::NotFound => "ENOENT",
        io::ErrorKind::PermissionDenied => "EACCES",
        io::ErrorKind::AlreadyExists => "EEXIST",
        io::ErrorKind::InvalidInput => "EINVAL",
        io::ErrorKind::InvalidData => "EINVAL",
        io::ErrorKind::TimedOut => "ETIMEDOUT",
        io::ErrorKind::Interrupted => "EINTR",
        io::ErrorKind::WouldBlock => "EWOULDBLOCK",
        _ => "EIO",
    }
}

fn fs_error_payload(error: io::Error) -> String {
    json!({
        "ok": false,
        "code": io_error_code(&error),
        "error": error.to_string()
    })
    .to_string()
}

fn system_time_to_millis(time: Result<SystemTime, io::Error>) -> Option<i64> {
    let value = time.ok()?;
    let dur = value.duration_since(UNIX_EPOCH).ok()?;
    Some(dur.as_millis() as i64)
}

fn normalize_encoding(encoding: Option<String>) -> String {
    encoding
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .replace('_', "-")
}

pub fn fs_read_file(path: String, encoding: Option<String>) -> String {
    match fs::read(&path) {
        Ok(bytes) => {
            let encoding = normalize_encoding(encoding);
            if encoding.is_empty() {
                json!({ "ok": true, "kind": "bytes", "data": bytes }).to_string()
            } else if encoding == "utf8" || encoding == "utf-8" {
                match String::from_utf8(bytes) {
                    Ok(text) => json!({ "ok": true, "kind": "text", "data": text }).to_string(),
                    Err(err) => json!({ "ok": false, "code": "EINVAL", "error": err.to_string() })
                        .to_string(),
                }
            } else {
                json!({
                    "ok": false,
                    "code": "EINVAL",
                    "error": format!("不支持的编码: {encoding}")
                })
                .to_string()
            }
        }
        Err(error) => fs_error_payload(error),
    }
}

fn parse_fs_write_payload(data_json: String, encoding: Option<String>) -> Result<Vec<u8>, String> {
    let value: Value = serde_json::from_str(&data_json).map_err(|e| e.to_string())?;
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .ok_or("缺少 kind 字段")?;

    if kind == "bytes" {
        let list = value
            .get("data")
            .and_then(Value::as_array)
            .ok_or("bytes 数据格式错误")?;
        let mut out = Vec::with_capacity(list.len());
        for item in list {
            let num = item.as_u64().ok_or("bytes 数据必须是 0-255 的整数")?;
            if num > 255 {
                return Err("bytes 数据必须在 0-255 范围内".to_string());
            }
            out.push(num as u8);
        }
        return Ok(out);
    }

    if kind == "text" {
        let text = value
            .get("data")
            .and_then(Value::as_str)
            .ok_or("text 数据格式错误")?;
        let encoding = normalize_encoding(encoding);
        if encoding.is_empty() || encoding == "utf8" || encoding == "utf-8" {
            return Ok(text.as_bytes().to_vec());
        }
        return Err(format!("不支持的编码: {encoding}"));
    }

    Err(format!("不支持的 kind: {kind}"))
}

pub fn fs_write_file(
    path: String,
    data_json: String,
    encoding: Option<String>,
    append: bool,
) -> String {
    let bytes = match parse_fs_write_payload(data_json, encoding) {
        Ok(bytes) => bytes,
        Err(error) => return json!({ "ok": false, "code": "EINVAL", "error": error }).to_string(),
    };

    let result = if append {
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| file.write_all(&bytes))
    } else {
        fs::write(&path, bytes)
    };

    match result {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_mkdir(path: String, recursive: bool) -> String {
    let result = if recursive {
        fs::create_dir_all(&path)
    } else {
        fs::create_dir(&path)
    };
    match result {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_readdir(path: String, with_file_types: bool) -> String {
    match fs::read_dir(&path) {
        Ok(read_dir) => {
            let mut entries = Vec::new();
            for entry in read_dir {
                match entry {
                    Ok(item) => {
                        let name = item.file_name().to_string_lossy().to_string();
                        if with_file_types {
                            match item.file_type() {
                                Ok(file_type) => entries.push(json!({
                                    "name": name,
                                    "isFile": file_type.is_file(),
                                    "isDirectory": file_type.is_dir(),
                                    "isSymbolicLink": file_type.is_symlink(),
                                })),
                                Err(error) => return fs_error_payload(error),
                            }
                        } else {
                            entries.push(Value::String(name));
                        }
                    }
                    Err(error) => return fs_error_payload(error),
                }
            }
            json!({ "ok": true, "entries": entries }).to_string()
        }
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_stat(path: String) -> String {
    match fs::metadata(&path) {
        Ok(metadata) => json!({
            "ok": true,
            "isFile": metadata.is_file(),
            "isDirectory": metadata.is_dir(),
            "isSymbolicLink": metadata.file_type().is_symlink(),
            "size": metadata.len(),
            "readonly": metadata.permissions().readonly(),
            "atimeMs": system_time_to_millis(metadata.accessed()),
            "mtimeMs": system_time_to_millis(metadata.modified()),
            "ctimeMs": system_time_to_millis(metadata.created())
        })
        .to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_access(path: String) -> String {
    match fs::metadata(&path) {
        Ok(_) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_unlink(path: String) -> String {
    match fs::remove_file(&path) {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_rm(path: String, recursive: bool, force: bool) -> String {
    let target = Path::new(&path);
    if !target.exists() {
        if force {
            return json!({ "ok": true }).to_string();
        }
        return json!({ "ok": false, "code": "ENOENT", "error": "文件或目录不存在" }).to_string();
    }

    let result = if target.is_dir() {
        if recursive {
            fs::remove_dir_all(target)
        } else {
            fs::remove_dir(target)
        }
    } else {
        fs::remove_file(target)
    };

    match result {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_rename(old_path: String, new_path: String) -> String {
    match fs::rename(&old_path, &new_path) {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_copy_file(src: String, dst: String) -> String {
    match fs::copy(&src, &dst) {
        Ok(bytes) => json!({ "ok": true, "bytesCopied": bytes }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_realpath(path: String) -> String {
    match fs::canonicalize(&path) {
        Ok(resolved) => {
            json!({ "ok": true, "path": resolved.to_string_lossy().to_string() }).to_string()
        }
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_lstat(path: String) -> String {
    match fs::symlink_metadata(&path) {
        Ok(metadata) => json!({
            "ok": true,
            "isFile": metadata.is_file(),
            "isDirectory": metadata.is_dir(),
            "isSymbolicLink": metadata.file_type().is_symlink(),
            "size": metadata.len(),
            "readonly": metadata.permissions().readonly(),
            "atimeMs": system_time_to_millis(metadata.accessed()),
            "mtimeMs": system_time_to_millis(metadata.modified()),
            "ctimeMs": system_time_to_millis(metadata.created())
        })
        .to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_readlink(path: String) -> String {
    match fs::read_link(&path) {
        Ok(target) => {
            json!({ "ok": true, "path": target.to_string_lossy().to_string() }).to_string()
        }
        Err(error) => fs_error_payload(error),
    }
}

#[cfg(unix)]
fn create_symlink_impl(target: &str, path: &str, _is_dir: bool) -> io::Result<()> {
    std::os::unix::fs::symlink(target, path)
}

#[cfg(windows)]
fn create_symlink_impl(target: &str, path: &str, is_dir: bool) -> io::Result<()> {
    if is_dir {
        std::os::windows::fs::symlink_dir(target, path)
    } else {
        std::os::windows::fs::symlink_file(target, path)
    }
}

#[cfg(not(any(unix, windows)))]
fn create_symlink_impl(_target: &str, _path: &str, _is_dir: bool) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "当前平台不支持符号链接",
    ))
}

pub fn fs_symlink(target: String, path: String, is_dir: bool) -> String {
    match create_symlink_impl(&target, &path, is_dir) {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_link(existing_path: String, new_path: String) -> String {
    match fs::hard_link(&existing_path, &new_path) {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_truncate(path: String, len: u64) -> String {
    let result = fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .and_then(|file| file.set_len(len));
    match result {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

#[cfg(unix)]
fn chmod_impl(path: &str, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, perms)
}

#[cfg(windows)]
fn chmod_impl(path: &str, mode: u32) -> io::Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_readonly((mode & 0o200) == 0);
    fs::set_permissions(path, perms)
}

#[cfg(not(any(unix, windows)))]
fn chmod_impl(_path: &str, _mode: u32) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "当前平台不支持 chmod",
    ))
}

pub fn fs_chmod(path: String, mode: u32) -> String {
    match chmod_impl(&path, mode) {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

pub fn fs_utimes(path: String, atime_millis: i64, mtime_millis: i64) -> String {
    let atime_secs = atime_millis.div_euclid(1000);
    let atime_nanos = (atime_millis.rem_euclid(1000) * 1_000_000) as u32;
    let mtime_secs = mtime_millis.div_euclid(1000);
    let mtime_nanos = (mtime_millis.rem_euclid(1000) * 1_000_000) as u32;
    let atime = FileTime::from_unix_time(atime_secs, atime_nanos);
    let mtime = FileTime::from_unix_time(mtime_secs, mtime_nanos);
    match set_file_times(&path, atime, mtime) {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dst_path)?;
        } else if file_type.is_symlink() {
            let target = fs::read_link(&src_path)?;
            create_symlink_impl(
                &target.to_string_lossy(),
                &dst_path.to_string_lossy(),
                target.is_dir(),
            )?;
        }
    }
    Ok(())
}

pub fn fs_cp(
    src: String,
    dst: String,
    recursive: bool,
    force: bool,
    error_on_exist: bool,
) -> String {
    let src_path = Path::new(&src);
    let dst_path = Path::new(&dst);

    if !src_path.exists() {
        return json!({ "ok": false, "code": "ENOENT", "error": "源路径不存在" }).to_string();
    }

    if dst_path.exists() {
        if error_on_exist {
            return json!({ "ok": false, "code": "EEXIST", "error": "目标路径已存在" }).to_string();
        }
        if !force {
            return json!({ "ok": false, "code": "EEXIST", "error": "目标路径已存在，且未启用 force" }).to_string();
        }
    }

    let result = if src_path.is_dir() {
        if !recursive {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "复制目录时必须启用 recursive",
            ))
        } else {
            copy_dir_recursive(src_path, dst_path)
        }
    } else {
        fs::copy(src_path, dst_path).map(|_| ())
    };

    match result {
        Ok(()) => json!({ "ok": true }).to_string(),
        Err(error) => fs_error_payload(error),
    }
}

static MKDTEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn fs_mkdtemp(prefix: String) -> String {
    for _ in 0..32 {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_else(|_| Duration::from_secs(0))
            .as_nanos();
        let seq = MKDTEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let candidate = format!("{prefix}{ts:016x}{seq:04x}");
        let path = PathBuf::from(candidate);
        match fs::create_dir(&path) {
            Ok(()) => {
                return json!({ "ok": true, "path": path.to_string_lossy().to_string() })
                    .to_string();
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return fs_error_payload(error),
        }
    }
    json!({ "ok": false, "code": "EEXIST", "error": "无法创建唯一临时目录" }).to_string()
}

#[cfg(test)]
pub fn run_async_script(script: &str) -> Result<String, Box<dyn std::error::Error>> {
    run_async_script_internal(script, false)
}

#[cfg(test)]
pub fn run_async_script_with_axios(script: &str) -> Result<String, Box<dyn std::error::Error>> {
    run_async_script_internal(script, true)
}

#[cfg(test)]
fn run_async_script_internal(
    script: &str,
    load_axios: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let runtime = Runtime::new()?;
    let context = Context::full(&runtime)?;

    context
        .with(|ctx| {
            install_host_bindings(&ctx)?;
            ctx.eval::<(), _>(WEB_POLYFILL)?;
            if load_axios {
                ctx.eval::<(), _>(AXIOS_BUNDLE)?;
            }
            let promise: Promise = ctx.eval(script)?;
            let result: String = promise.finish()?;
            Ok::<String, rquickjs::Error>(result)
        })
        .map_err(|e| e.into())
}
