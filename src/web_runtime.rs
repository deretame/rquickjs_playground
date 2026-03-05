use serde_json::Map;
use serde_json::{Value, json};
use anyhow::{Context as AnyhowContext, Result as AnyResult, anyhow};
use base64::Engine as Base64Engine;
use base64::engine::general_purpose::{STANDARD as BASE64_STANDARD, URL_SAFE as BASE64_URL_SAFE};
use serde::Deserialize;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use log::Level;
use reqwest::{Client, Method};

use filetime::{FileTime, set_file_times};
use rquickjs::{Ctx, Promise, function::Func};
use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime as TokioRuntime};
use tokio::sync::Semaphore;
use tokio::task::JoinHandle;
use tokio::time::timeout;
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
    include_str!("../js/64_cache.js"),
    "\n",
    include_str!("../js/65_console.js"),
    "\n",
    include_str!("../js/99_exports.js"),
    "\n"
);

#[cfg(test)]
pub const AXIOS_BUNDLE: &str = include_str!("../vendor/axios.min.js");

#[cfg(not(test))]
pub const AXIOS_BUNDLE: &str = "";

pub fn install_host_bindings(ctx: &Ctx<'_>) -> Result<(), rquickjs::Error> {
    let globals = ctx.globals();
    globals.set("__http_request_start", Func::from(http_request_start))?;
    globals.set("__http_request_try_take", Func::from(http_request_try_take))?;
    globals.set("__http_request_drop", Func::from(http_request_drop))?;
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
    globals.set("__wasi_run_start", Func::from(wasi_run_start))?;
    globals.set("__wasi_run_try_take", Func::from(wasi_run_try_take))?;
    globals.set("__wasi_run_drop", Func::from(wasi_run_drop))?;
    globals.set("__cache_set", Func::from(cache_set))?;
    globals.set("__cache_set_if_absent", Func::from(cache_set_if_absent))?;
    globals.set("__cache_compare_and_set", Func::from(cache_compare_and_set))?;
    globals.set("__cache_get", Func::from(cache_get))?;
    globals.set("__cache_delete", Func::from(cache_delete))?;
    globals.set("__cache_clear", Func::from(cache_clear))?;
    globals.set("__log_emit", Func::from(log_emit))?;
    globals.set("__runtime_stats", Func::from(runtime_stats))?;
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
    globals.set("__fs_task_start", Func::from(fs_task_start))?;
    globals.set("__fs_task_try_take", Func::from(fs_task_try_take))?;
    globals.set("__fs_task_drop", Func::from(fs_task_drop))?;
    Ok(())
}

static HTTP_REQ_ID: AtomicU64 = AtomicU64::new(1);
static HTTP_REQ_POOL: OnceLock<Mutex<HashMap<u64, PendingTask>>> = OnceLock::new();
static FS_REQ_ID: AtomicU64 = AtomicU64::new(1);
static FS_REQ_POOL: OnceLock<Mutex<HashMap<u64, PendingTask>>> = OnceLock::new();
static WASI_REQ_ID: AtomicU64 = AtomicU64::new(1);
static WASI_REQ_POOL: OnceLock<Mutex<HashMap<u64, PendingTask>>> = OnceLock::new();
static WASI_ENGINE: OnceLock<Engine> = OnceLock::new();
static WASI_MODULE_CACHE: OnceLock<Mutex<HashMap<Vec<u8>, Module>>> = OnceLock::new();
static WASI_MODULE_CACHE_ORDER: OnceLock<Mutex<VecDeque<Vec<u8>>>> = OnceLock::new();
static WASI_LINKER: OnceLock<Linker<wasmtime_wasi::p1::WasiP1Ctx>> = OnceLock::new();
static WASI_CACHE_HITS: AtomicU64 = AtomicU64::new(0);
static WASI_CACHE_MISSES: AtomicU64 = AtomicU64::new(0);
static WASI_CACHE_EVICTIONS: AtomicU64 = AtomicU64::new(0);
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();
static HOST_ASYNC_RT: OnceLock<TokioRuntime> = OnceLock::new();
static HTTP_IO_SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();
static FS_IO_SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();
static WASI_IO_SEM: OnceLock<Arc<Semaphore>> = OnceLock::new();
static HTTP_STALE_DROPS: AtomicU64 = AtomicU64::new(0);
static FS_STALE_DROPS: AtomicU64 = AtomicU64::new(0);
static WASI_STALE_DROPS: AtomicU64 = AtomicU64::new(0);
static CACHE_TX: OnceLock<mpsc::Sender<CacheCommand>> = OnceLock::new();
static LOG_TX: OnceLock<mpsc::Sender<LogEvent>> = OnceLock::new();
static LOG_ENQUEUED: AtomicU64 = AtomicU64::new(0);
static LOG_WRITTEN: AtomicU64 = AtomicU64::new(0);
static LOG_DROPPED: AtomicU64 = AtomicU64::new(0);
static LOG_ERRORS: AtomicU64 = AtomicU64::new(0);
static LOG_PENDING: AtomicU64 = AtomicU64::new(0);

struct PendingTask {
    rx: mpsc::Receiver<String>,
    task: JoinHandle<()>,
    created_at: Instant,
}

enum CacheCommand {
    Set {
        key: String,
        value_json: String,
        reply_tx: mpsc::Sender<String>,
    },
    SetIfAbsent {
        key: String,
        value_json: String,
        reply_tx: mpsc::Sender<String>,
    },
    CompareAndSet {
        key: String,
        expected_json: String,
        value_json: String,
        reply_tx: mpsc::Sender<String>,
    },
    Get {
        key: String,
        reply_tx: mpsc::Sender<String>,
    },
    Delete {
        key: String,
        reply_tx: mpsc::Sender<String>,
    },
    Clear {
        reply_tx: mpsc::Sender<String>,
    },
}

struct LogEvent {
    level: String,
    message: String,
    ts_ms: u128,
}

fn http_req_pool() -> &'static Mutex<HashMap<u64, PendingTask>> {
    HTTP_REQ_POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn fs_req_pool() -> &'static Mutex<HashMap<u64, PendingTask>> {
    FS_REQ_POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn wasi_req_pool() -> &'static Mutex<HashMap<u64, PendingTask>> {
    WASI_REQ_POOL.get_or_init(|| Mutex::new(HashMap::new()))
}

fn wasi_engine() -> &'static Engine {
    WASI_ENGINE.get_or_init(Engine::default)
}

fn wasi_module_cache() -> &'static Mutex<HashMap<Vec<u8>, Module>> {
    WASI_MODULE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn wasi_module_cache_order() -> &'static Mutex<VecDeque<Vec<u8>>> {
    WASI_MODULE_CACHE_ORDER.get_or_init(|| Mutex::new(VecDeque::new()))
}

fn wasi_linker() -> AnyResult<&'static Linker<wasmtime_wasi::p1::WasiP1Ctx>> {
    if let Some(linker) = WASI_LINKER.get() {
        return Ok(linker);
    }

    let mut linker: Linker<wasmtime_wasi::p1::WasiP1Ctx> = Linker::new(wasi_engine());
    wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |s| s)
        .map_err(|e| anyhow!("注册 WASI linker 失败: {e}"))?;

    match WASI_LINKER.set(linker) {
        Ok(()) => Ok(WASI_LINKER
            .get()
            .expect("wasi linker 初始化后必须可读取")),
        Err(_linker) => Ok(WASI_LINKER
            .get()
            .expect("wasi linker 并发初始化后必须可读取")),
    }
}

const HTTP_MAX_IN_FLIGHT: usize = 256;
const FS_MAX_IN_FLIGHT: usize = 128;
const WASI_MAX_IN_FLIGHT: usize = 32;
const HTTP_OFFLOAD_BODY_HEADER: &str = "x-rquickjs-host-offload-binary-v1";
const HTTP_WASI_TRANSFORM_HEADER: &str = "x-rquickjs-host-wasi-transform-b64-v1";
const HTTP_MAX_PENDING: usize = 4096;
const FS_MAX_PENDING: usize = 4096;
const WASI_MAX_PENDING: usize = 1024;
const PENDING_TASK_TTL: Duration = Duration::from_secs(120);

fn http_io_sem() -> &'static Arc<Semaphore> {
    HTTP_IO_SEM.get_or_init(|| Arc::new(Semaphore::new(HTTP_MAX_IN_FLIGHT)))
}

fn fs_io_sem() -> &'static Arc<Semaphore> {
    FS_IO_SEM.get_or_init(|| Arc::new(Semaphore::new(FS_MAX_IN_FLIGHT)))
}

fn wasi_io_sem() -> &'static Arc<Semaphore> {
    WASI_IO_SEM.get_or_init(|| Arc::new(Semaphore::new(WASI_MAX_IN_FLIGHT)))
}

fn cleanup_stale_pending(pool: &mut HashMap<u64, PendingTask>, dropped_counter: &AtomicU64) {
    let now = Instant::now();
    let stale_ids: Vec<u64> = pool
        .iter()
        .filter_map(|(id, pending)| {
            if now.duration_since(pending.created_at) > PENDING_TASK_TTL {
                Some(*id)
            } else {
                None
            }
        })
        .collect();

    for id in stale_ids {
        if let Some(pending) = pool.remove(&id) {
            pending.task.abort();
            dropped_counter.fetch_add(1, Ordering::Relaxed);
        }
    }
}

fn header_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WasiTransformPlan {
    module_id: u64,
    function: Option<String>,
    args: Option<Value>,
    js_process: Option<bool>,
    output_type: String,
}

fn parse_wasi_transform_plan(raw_b64: &str) -> AnyResult<WasiTransformPlan> {
    let raw = raw_b64.trim();
    let decoded = BASE64_URL_SAFE
        .decode(raw)
        .or_else(|_| BASE64_STANDARD.decode(raw))
        .context("base64 解码 wasi transform plan 失败")?;
    let json_text = String::from_utf8(decoded).context("wasi transform plan 不是有效 UTF-8")?;
    serde_json::from_str::<WasiTransformPlan>(&json_text).context("解析 wasi transform plan JSON 失败")
}

fn build_wasi_argv_json(plan: &WasiTransformPlan) -> AnyResult<Option<String>> {
    let mut argv: Vec<String> = Vec::new();
    if let Some(function) = &plan.function {
        argv.push("--fn".to_string());
        argv.push(function.clone());
    }
    if let Some(args) = &plan.args {
        argv.push("--args-json".to_string());
        argv.push(serde_json::to_string(args).context("序列化 wasi args 失败")?);
    }
    if argv.is_empty() {
        Ok(None)
    } else {
        Ok(Some(serde_json::to_string(&argv).context("序列化 wasi argv 失败")?))
    }
}

async fn run_wasi_transform_once(plan: &WasiTransformPlan, input: Vec<u8>) -> AnyResult<Vec<u8>> {
    if !plan.output_type.eq_ignore_ascii_case("binary") {
        return Err(anyhow!(
            "当前仅支持 outputType=binary，收到: {}",
            plan.output_type
        ));
    }

    let stdin_id = native_buffer_put_raw(input);
    let args_json = build_wasi_argv_json(plan)?;
    let module_id = plan.module_id;
    let raw = tokio::task::spawn_blocking(move || {
        wasi_run_inner(module_id, Some(stdin_id), args_json, false)
    })
    .await
    .context("执行 wasi transform 任务失败")?;
    let payload = parse_host_ok_payload(raw)?;

    let exit_code = payload
        .get("exitCode")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let stderr_id = payload
        .get("stderrId")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("wasi 返回缺少 stderrId"))?;
    let stderr = native_buffer_take_raw(stderr_id).unwrap_or_default();
    let stderr_text = String::from_utf8_lossy(&stderr).to_string();

    if exit_code != 0 {
        return Err(anyhow!(
            "wasi 执行失败，exitCode={exit_code}, stderr={stderr_text}"
        ));
    }

    let stdout_id = payload
        .get("stdoutId")
        .and_then(Value::as_u64)
        .ok_or_else(|| anyhow!("wasi 返回缺少 stdoutId"))?;
    let out = native_buffer_take_raw(stdout_id).ok_or_else(|| anyhow!("wasi stdout buffer 不存在"))?;
    Ok(out)
}

fn cache_sender() -> &'static mpsc::Sender<CacheCommand> {
    CACHE_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<CacheCommand>();
        thread::Builder::new()
            .name("rquickjs-cache-worker".to_string())
            .spawn(move || {
                let mut map: HashMap<String, Value> = HashMap::new();
                while let Ok(cmd) = rx.recv() {
                    match cmd {
                        CacheCommand::Set {
                            key,
                            value_json,
                            reply_tx,
                        } => {
                            let out = match serde_json::from_str::<Value>(&value_json) {
                                Ok(value) => {
                                    map.insert(key, value);
                                    json!({ "ok": true }).to_string()
                                }
                                Err(e) => {
                                    json!({ "ok": false, "error": format!("value 不是合法 JSON: {e}") }).to_string()
                                }
                            };
                            let _ = reply_tx.send(out);
                        }
                        CacheCommand::SetIfAbsent {
                            key,
                            value_json,
                            reply_tx,
                        } => {
                            let out = match serde_json::from_str::<Value>(&value_json) {
                                Ok(value) => {
                                    if map.contains_key(&key) {
                                        json!({ "ok": true, "inserted": false }).to_string()
                                    } else {
                                        map.insert(key, value);
                                        json!({ "ok": true, "inserted": true }).to_string()
                                    }
                                }
                                Err(e) => {
                                    json!({ "ok": false, "error": format!("value 不是合法 JSON: {e}") }).to_string()
                                }
                            };
                            let _ = reply_tx.send(out);
                        }
                        CacheCommand::CompareAndSet {
                            key,
                            expected_json,
                            value_json,
                            reply_tx,
                        } => {
                            let expected = match serde_json::from_str::<Value>(&expected_json) {
                                Ok(v) => v,
                                Err(e) => {
                                    let _ = reply_tx.send(
                                        json!({ "ok": false, "error": format!("expected 不是合法 JSON: {e}") })
                                            .to_string(),
                                    );
                                    continue;
                                }
                            };
                            let next = match serde_json::from_str::<Value>(&value_json) {
                                Ok(v) => v,
                                Err(e) => {
                                    let _ = reply_tx.send(
                                        json!({ "ok": false, "error": format!("value 不是合法 JSON: {e}") })
                                            .to_string(),
                                    );
                                    continue;
                                }
                            };

                            let updated = match map.get(&key) {
                                Some(current) if *current == expected => {
                                    map.insert(key, next);
                                    true
                                }
                                _ => false,
                            };
                            let _ = reply_tx.send(json!({ "ok": true, "updated": updated }).to_string());
                        }
                        CacheCommand::Get { key, reply_tx } => {
                            let out = match map.get(&key) {
                                Some(v) => {
                                    json!({ "ok": true, "found": true, "value": v.clone() }).to_string()
                                }
                                None => {
                                    json!({ "ok": true, "found": false, "value": Value::Null }).to_string()
                                }
                            };
                            let _ = reply_tx.send(out);
                        }
                        CacheCommand::Delete { key, reply_tx } => {
                            let existed = map.remove(&key).is_some();
                            let _ = reply_tx.send(json!({ "ok": true, "deleted": existed }).to_string());
                        }
                        CacheCommand::Clear { reply_tx } => {
                            map.clear();
                            let _ = reply_tx.send(json!({ "ok": true }).to_string());
                        }
                    }
                }
            })
            .expect("创建 cache worker 失败");
        tx
    })
}

const LOG_MAX_PENDING: u64 = 16_384;

fn log_sender() -> &'static mpsc::Sender<LogEvent> {
    LOG_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<LogEvent>();
        thread::Builder::new()
            .name("rquickjs-log-worker".to_string())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    LOG_PENDING.fetch_sub(1, Ordering::Relaxed);
                    let line = format!("[qjs:{}:{}] {}", event.ts_ms, event.level, event.message);
                    let level = match event.level.as_str() {
                        "error" => Level::Error,
                        "warn" => Level::Warn,
                        "info" => Level::Info,
                        "debug" => Level::Debug,
                        _ => Level::Trace,
                    };
                    log::log!(level, "{line}");
                    LOG_WRITTEN.fetch_add(1, Ordering::Relaxed);
                }
            })
            .expect("创建 log worker 失败");
        tx
    })
}

fn host_async_runtime() -> &'static TokioRuntime {
    HOST_ASYNC_RT.get_or_init(|| {
        TokioRuntimeBuilder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .thread_name("rquickjs-host-async")
            .build()
            .expect("创建 Host Tokio runtime 失败")
    })
}

fn http_client() -> AnyResult<&'static Client> {
    if let Some(client) = HTTP_CLIENT.get() {
        return Ok(client);
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("创建 HTTP client 失败")?;

    match HTTP_CLIENT.set(client) {
        Ok(()) => Ok(HTTP_CLIENT.get().expect("HTTP client 初始化后必须可读取")),
        Err(_client) => Ok(HTTP_CLIENT
            .get()
            .expect("HTTP client 并发初始化后必须可读取")),
    }
}

pub fn http_request_start(
    method: String,
    url: String,
    headers_json: String,
    body: Option<String>,
) -> String {
    {
        let mut pool = http_req_pool().lock().expect("http 请求池加锁失败");
        cleanup_stale_pending(&mut pool, &HTTP_STALE_DROPS);
        if pool.len() >= HTTP_MAX_PENDING {
            return json!({ "ok": false, "error": "http pending 队列已满" }).to_string();
        }
    }

    let id = HTTP_REQ_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel::<String>();
    let sem = Arc::clone(http_io_sem());

    let task = host_async_runtime().spawn(async move {
        let permit = match timeout(Duration::from_secs(15), sem.acquire_owned()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                let _ = tx.send(json!({ "ok": false, "error": "http 并发控制器不可用" }).to_string());
                return;
            }
            Err(_) => {
                let _ = tx.send(json!({ "ok": false, "error": "http 等待并发许可超时" }).to_string());
                return;
            }
        };
        let payload = match http_request_inner_async(method, url, headers_json, body).await {
            Ok(payload) => payload,
            Err(error) => json!({ "ok": false, "error": format!("{error}") }).to_string(),
        };
        drop(permit);
        let _ = tx.send(payload);
    });

    {
        let mut pool = http_req_pool().lock().expect("http 请求池加锁失败");
        pool.insert(
            id,
            PendingTask {
                rx,
                task,
                created_at: Instant::now(),
            },
        );
    }

    json!({ "ok": true, "id": id }).to_string()
}

pub fn http_request_try_take(id: u64) -> String {
    let mut pool = http_req_pool().lock().expect("http 请求池加锁失败");
    cleanup_stale_pending(&mut pool, &HTTP_STALE_DROPS);
    let Some(pending) = pool.get_mut(&id) else {
        return json!({ "ok": false, "error": "request id 不存在" }).to_string();
    };

    match pending.rx.try_recv() {
        Ok(result) => {
            pool.remove(&id);
            json!({ "ok": true, "done": true, "result": result }).to_string()
        }
        Err(TryRecvError::Empty) => json!({ "ok": true, "done": false }).to_string(),
        Err(TryRecvError::Disconnected) => {
            pool.remove(&id);
            json!({ "ok": false, "error": "request 执行线程异常退出" }).to_string()
        }
    }
}

pub fn http_request_drop(id: u64) -> String {
    let mut pool = http_req_pool().lock().expect("http 请求池加锁失败");
    let existed = if let Some(pending) = pool.remove(&id) {
        pending.task.abort();
        true
    } else {
        false
    };
    json!({ "ok": true, "dropped": existed }).to_string()
}

pub fn fs_task_start(op: String, args_json: String) -> String {
    {
        let mut pool = fs_req_pool().lock().expect("fs 请求池加锁失败");
        cleanup_stale_pending(&mut pool, &FS_STALE_DROPS);
        if pool.len() >= FS_MAX_PENDING {
            return json!({ "ok": false, "error": "fs pending 队列已满" }).to_string();
        }
    }

    let id = FS_REQ_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel::<String>();
    let sem = Arc::clone(fs_io_sem());

    let task = host_async_runtime().spawn(async move {
        let permit = match timeout(Duration::from_secs(15), sem.acquire_owned()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                let _ = tx.send(json!({ "ok": false, "error": "fs 并发控制器不可用" }).to_string());
                return;
            }
            Err(_) => {
                let _ = tx.send(json!({ "ok": false, "error": "fs 等待并发许可超时" }).to_string());
                return;
            }
        };
        let payload = tokio::task::spawn_blocking(move || fs_task_dispatch(op, args_json))
            .await
            .unwrap_or_else(|e| json!({ "ok": false, "error": e.to_string() }).to_string());
        drop(permit);
        let _ = tx.send(payload);
    });

    {
        let mut pool = fs_req_pool().lock().expect("fs 请求池加锁失败");
        pool.insert(
            id,
            PendingTask {
                rx,
                task,
                created_at: Instant::now(),
            },
        );
    }

    json!({ "ok": true, "id": id }).to_string()
}

pub fn fs_task_try_take(id: u64) -> String {
    let mut pool = fs_req_pool().lock().expect("fs 请求池加锁失败");
    cleanup_stale_pending(&mut pool, &FS_STALE_DROPS);
    let Some(pending) = pool.get_mut(&id) else {
        return json!({ "ok": false, "error": "request id 不存在" }).to_string();
    };

    match pending.rx.try_recv() {
        Ok(result) => {
            pool.remove(&id);
            json!({ "ok": true, "done": true, "result": result }).to_string()
        }
        Err(TryRecvError::Empty) => json!({ "ok": true, "done": false }).to_string(),
        Err(TryRecvError::Disconnected) => {
            pool.remove(&id);
            json!({ "ok": false, "error": "fs 执行任务异常退出" }).to_string()
        }
    }
}

pub fn fs_task_drop(id: u64) -> String {
    let mut pool = fs_req_pool().lock().expect("fs 请求池加锁失败");
    let existed = if let Some(pending) = pool.remove(&id) {
        pending.task.abort();
        true
    } else {
        false
    };
    json!({ "ok": true, "dropped": existed }).to_string()
}

fn fs_task_dispatch(op: String, args_json: String) -> String {
    let args: Vec<Value> = match serde_json::from_str(&args_json) {
        Ok(v) => v,
        Err(e) => {
            return json!({ "ok": false, "code": "EINVAL", "error": e.to_string() }).to_string();
        }
    };

    let arg_str = |idx: usize, name: &str| -> Result<String, String> {
        args.get(idx)
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .ok_or_else(|| format!("参数 {name} 必须是字符串"))
    };
    let arg_bool = |idx: usize, name: &str| -> Result<bool, String> {
        args.get(idx)
            .and_then(Value::as_bool)
            .ok_or_else(|| format!("参数 {name} 必须是布尔值"))
    };
    let arg_u64 = |idx: usize, name: &str| -> Result<u64, String> {
        args.get(idx)
            .and_then(Value::as_u64)
            .ok_or_else(|| format!("参数 {name} 必须是非负整数"))
    };
    let arg_u32 = |idx: usize, name: &str| -> Result<u32, String> {
        arg_u64(idx, name)
            .and_then(|v| u32::try_from(v).map_err(|_| format!("参数 {name} 超出 u32 范围")))
    };
    let arg_i64 = |idx: usize, name: &str| -> Result<i64, String> {
        args.get(idx)
            .and_then(Value::as_i64)
            .ok_or_else(|| format!("参数 {name} 必须是整数"))
    };
    let arg_opt_str = |idx: usize| -> Option<String> {
        args.get(idx).and_then(|v| {
            if v.is_null() {
                None
            } else {
                v.as_str().map(ToString::to_string)
            }
        })
    };

    match op.as_str() {
        "readFile" => match arg_str(0, "path") {
            Ok(path) => fs_read_file(path, arg_opt_str(1)),
            Err(e) => json!({ "ok": false, "code": "EINVAL", "error": e }).to_string(),
        },
        "writeFile" => match (
            arg_str(0, "path"),
            arg_str(1, "dataJson"),
            arg_opt_str(2),
            arg_bool(3, "append"),
        ) {
            (Ok(path), Ok(data_json), encoding, Ok(append)) => {
                fs_write_file(path, data_json, encoding, append)
            }
            _ => {
                json!({ "ok": false, "code": "EINVAL", "error": "writeFile 参数无效" }).to_string()
            }
        },
        "mkdir" => match (arg_str(0, "path"), arg_bool(1, "recursive")) {
            (Ok(path), Ok(recursive)) => fs_mkdir(path, recursive),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "mkdir 参数无效" }).to_string(),
        },
        "readdir" => match (arg_str(0, "path"), arg_bool(1, "withFileTypes")) {
            (Ok(path), Ok(with_file_types)) => fs_readdir(path, with_file_types),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "readdir 参数无效" }).to_string(),
        },
        "stat" => match arg_str(0, "path") {
            Ok(path) => fs_stat(path),
            Err(e) => json!({ "ok": false, "code": "EINVAL", "error": e }).to_string(),
        },
        "lstat" => match arg_str(0, "path") {
            Ok(path) => fs_lstat(path),
            Err(e) => json!({ "ok": false, "code": "EINVAL", "error": e }).to_string(),
        },
        "access" => match arg_str(0, "path") {
            Ok(path) => fs_access(path),
            Err(e) => json!({ "ok": false, "code": "EINVAL", "error": e }).to_string(),
        },
        "unlink" => match arg_str(0, "path") {
            Ok(path) => fs_unlink(path),
            Err(e) => json!({ "ok": false, "code": "EINVAL", "error": e }).to_string(),
        },
        "rm" => match (
            arg_str(0, "path"),
            arg_bool(1, "recursive"),
            arg_bool(2, "force"),
        ) {
            (Ok(path), Ok(recursive), Ok(force)) => fs_rm(path, recursive, force),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "rm 参数无效" }).to_string(),
        },
        "rename" => match (arg_str(0, "oldPath"), arg_str(1, "newPath")) {
            (Ok(old_path), Ok(new_path)) => fs_rename(old_path, new_path),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "rename 参数无效" }).to_string(),
        },
        "copyFile" => match (arg_str(0, "src"), arg_str(1, "dst")) {
            (Ok(src), Ok(dst)) => fs_copy_file(src, dst),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "copyFile 参数无效" }).to_string(),
        },
        "cp" => match (
            arg_str(0, "src"),
            arg_str(1, "dst"),
            arg_bool(2, "recursive"),
            arg_bool(3, "force"),
            arg_bool(4, "errorOnExist"),
        ) {
            (Ok(src), Ok(dst), Ok(recursive), Ok(force), Ok(error_on_exist)) => {
                fs_cp(src, dst, recursive, force, error_on_exist)
            }
            _ => json!({ "ok": false, "code": "EINVAL", "error": "cp 参数无效" }).to_string(),
        },
        "realpath" => match arg_str(0, "path") {
            Ok(path) => fs_realpath(path),
            Err(e) => json!({ "ok": false, "code": "EINVAL", "error": e }).to_string(),
        },
        "readlink" => match arg_str(0, "path") {
            Ok(path) => fs_readlink(path),
            Err(e) => json!({ "ok": false, "code": "EINVAL", "error": e }).to_string(),
        },
        "symlink" => match (
            arg_str(0, "target"),
            arg_str(1, "path"),
            arg_bool(2, "isDir"),
        ) {
            (Ok(target), Ok(path), Ok(is_dir)) => fs_symlink(target, path, is_dir),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "symlink 参数无效" }).to_string(),
        },
        "link" => match (arg_str(0, "existingPath"), arg_str(1, "newPath")) {
            (Ok(existing_path), Ok(new_path)) => fs_link(existing_path, new_path),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "link 参数无效" }).to_string(),
        },
        "truncate" => match (arg_str(0, "path"), arg_u64(1, "len")) {
            (Ok(path), Ok(len)) => fs_truncate(path, len),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "truncate 参数无效" }).to_string(),
        },
        "chmod" => match (arg_str(0, "path"), arg_u32(1, "mode")) {
            (Ok(path), Ok(mode)) => fs_chmod(path, mode),
            _ => json!({ "ok": false, "code": "EINVAL", "error": "chmod 参数无效" }).to_string(),
        },
        "utimes" => match (arg_str(0, "path"), arg_i64(1, "atime"), arg_i64(2, "mtime")) {
            (Ok(path), Ok(atime_millis), Ok(mtime_millis)) => {
                fs_utimes(path, atime_millis, mtime_millis)
            }
            _ => json!({ "ok": false, "code": "EINVAL", "error": "utimes 参数无效" }).to_string(),
        },
        "mkdtemp" => match arg_str(0, "prefix") {
            Ok(prefix) => fs_mkdtemp(prefix),
            Err(e) => json!({ "ok": false, "code": "EINVAL", "error": e }).to_string(),
        },
        _ => {
            json!({ "ok": false, "code": "EINVAL", "error": format!("不支持的 fs 异步操作: {op}") })
                .to_string()
        }
    }
}

pub fn cache_set(key: String, value_json: String) -> String {
    let (reply_tx, reply_rx) = mpsc::channel::<String>();
    if let Err(e) = cache_sender().send(CacheCommand::Set {
        key,
        value_json,
        reply_tx,
    }) {
        return json!({ "ok": false, "error": format!("cache worker 不可用: {e}") }).to_string();
    }
    match reply_rx.recv() {
        Ok(raw) => raw,
        Err(e) => {
            json!({ "ok": false, "error": format!("cache worker 响应失败: {e}") }).to_string()
        }
    }
}

pub fn cache_set_if_absent(key: String, value_json: String) -> String {
    let (reply_tx, reply_rx) = mpsc::channel::<String>();
    if let Err(e) = cache_sender().send(CacheCommand::SetIfAbsent {
        key,
        value_json,
        reply_tx,
    }) {
        return json!({ "ok": false, "error": format!("cache worker 不可用: {e}") }).to_string();
    }
    match reply_rx.recv() {
        Ok(raw) => raw,
        Err(e) => {
            json!({ "ok": false, "error": format!("cache worker 响应失败: {e}") }).to_string()
        }
    }
}

pub fn cache_compare_and_set(key: String, expected_json: String, value_json: String) -> String {
    let (reply_tx, reply_rx) = mpsc::channel::<String>();
    if let Err(e) = cache_sender().send(CacheCommand::CompareAndSet {
        key,
        expected_json,
        value_json,
        reply_tx,
    }) {
        return json!({ "ok": false, "error": format!("cache worker 不可用: {e}") }).to_string();
    }
    match reply_rx.recv() {
        Ok(raw) => raw,
        Err(e) => {
            json!({ "ok": false, "error": format!("cache worker 响应失败: {e}") }).to_string()
        }
    }
}

pub fn cache_get(key: String) -> String {
    let (reply_tx, reply_rx) = mpsc::channel::<String>();
    if let Err(e) = cache_sender().send(CacheCommand::Get { key, reply_tx }) {
        return json!({ "ok": false, "error": format!("cache worker 不可用: {e}") }).to_string();
    }
    match reply_rx.recv() {
        Ok(raw) => raw,
        Err(e) => {
            json!({ "ok": false, "error": format!("cache worker 响应失败: {e}") }).to_string()
        }
    }
}

pub fn cache_delete(key: String) -> String {
    let (reply_tx, reply_rx) = mpsc::channel::<String>();
    if let Err(e) = cache_sender().send(CacheCommand::Delete { key, reply_tx }) {
        return json!({ "ok": false, "error": format!("cache worker 不可用: {e}") }).to_string();
    }
    match reply_rx.recv() {
        Ok(raw) => raw,
        Err(e) => {
            json!({ "ok": false, "error": format!("cache worker 响应失败: {e}") }).to_string()
        }
    }
}

pub fn cache_clear() -> String {
    let (reply_tx, reply_rx) = mpsc::channel::<String>();
    if let Err(e) = cache_sender().send(CacheCommand::Clear { reply_tx }) {
        return json!({ "ok": false, "error": format!("cache worker 不可用: {e}") }).to_string();
    }
    match reply_rx.recv() {
        Ok(raw) => raw,
        Err(e) => {
            json!({ "ok": false, "error": format!("cache worker 响应失败: {e}") }).to_string()
        }
    }
}

pub fn log_emit(level: String, message: String) -> String {
    let level_norm = level.trim().to_ascii_lowercase();
    let level = if level_norm.is_empty() {
        "log".to_string()
    } else {
        level_norm
    };

    let current = LOG_PENDING.load(Ordering::Relaxed);
    if current >= LOG_MAX_PENDING {
        LOG_DROPPED.fetch_add(1, Ordering::Relaxed);
        return json!({ "ok": true, "dropped": true }).to_string();
    }

    LOG_PENDING.fetch_add(1, Ordering::Relaxed);

    let event = LogEvent {
        level,
        message,
        ts_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default(),
    };

    if let Err(e) = log_sender().send(event) {
        LOG_PENDING.fetch_sub(1, Ordering::Relaxed);
        LOG_ERRORS.fetch_add(1, Ordering::Relaxed);
        return json!({ "ok": false, "error": format!("log worker 不可用: {e}") }).to_string();
    }

    LOG_ENQUEUED.fetch_add(1, Ordering::Relaxed);
    json!({ "ok": true, "dropped": false }).to_string()
}

pub fn runtime_stats() -> String {
    let mut http_pending = http_req_pool()
        .lock()
        .map(|m| m.len())
        .unwrap_or_default();
    let mut fs_pending = fs_req_pool().lock().map(|m| m.len()).unwrap_or_default();
    let mut wasi_pending = wasi_req_pool().lock().map(|m| m.len()).unwrap_or_default();

    if let Ok(mut pool) = http_req_pool().lock() {
        cleanup_stale_pending(&mut pool, &HTTP_STALE_DROPS);
        http_pending = pool.len();
    }
    if let Ok(mut pool) = fs_req_pool().lock() {
        cleanup_stale_pending(&mut pool, &FS_STALE_DROPS);
        fs_pending = pool.len();
    }
    if let Ok(mut pool) = wasi_req_pool().lock() {
        cleanup_stale_pending(&mut pool, &WASI_STALE_DROPS);
        wasi_pending = pool.len();
    }

    let wasi_cache_size = wasi_module_cache()
        .lock()
        .map(|m| m.len())
        .unwrap_or_default();
    let http_available = http_io_sem().available_permits();
    let fs_available = fs_io_sem().available_permits();
    let wasi_available = wasi_io_sem().available_permits();

    json!({
        "ok": true,
        "limits": {
            "pending": {
                "http": HTTP_MAX_PENDING,
                "fs": FS_MAX_PENDING,
                "wasi": WASI_MAX_PENDING,
            },
            "inFlight": {
                "http": HTTP_MAX_IN_FLIGHT,
                "fs": FS_MAX_IN_FLIGHT,
                "wasi": WASI_MAX_IN_FLIGHT,
            }
        },
        "pending": {
            "http": http_pending,
            "fs": fs_pending,
            "wasi": wasi_pending,
        },
        "permits": {
            "httpAvailable": http_available,
            "fsAvailable": fs_available,
            "wasiAvailable": wasi_available,
        },
        "staleDrops": {
            "http": HTTP_STALE_DROPS.load(Ordering::Relaxed),
            "fs": FS_STALE_DROPS.load(Ordering::Relaxed),
            "wasi": WASI_STALE_DROPS.load(Ordering::Relaxed),
        },
        "logs": {
            "pending": LOG_PENDING.load(Ordering::Relaxed),
            "pendingCapacity": LOG_MAX_PENDING,
            "enqueued": LOG_ENQUEUED.load(Ordering::Relaxed),
            "written": LOG_WRITTEN.load(Ordering::Relaxed),
            "dropped": LOG_DROPPED.load(Ordering::Relaxed),
            "errors": LOG_ERRORS.load(Ordering::Relaxed),
        },
        "wasi": {
            "cacheSize": wasi_cache_size,
            "cacheCapacity": WASI_MODULE_CACHE_MAX_ENTRIES,
            "cacheHits": WASI_CACHE_HITS.load(Ordering::Relaxed),
            "cacheMisses": WASI_CACHE_MISSES.load(Ordering::Relaxed),
            "cacheEvictions": WASI_CACHE_EVICTIONS.load(Ordering::Relaxed),
        }
    })
    .to_string()
}

async fn http_request_inner_async(
    method: String,
    url: String,
    headers_json: String,
    body: Option<String>,
) -> AnyResult<String> {
    let method = Method::from_bytes(method.as_bytes()).context("解析 HTTP method 失败")?;
    let mut headers_map = Map::new();
    let headers_value: Value =
        serde_json::from_str(&headers_json).context("解析 HTTP headers JSON 失败")?;
    let client = http_client()?;
    let mut offload_body_to_native = false;
    let mut wasi_transform_plan: Option<WasiTransformPlan> = None;

    let mut builder = client.request(method, &url);

    if let Value::Object(obj) = headers_value {
        for (key, value) in obj {
            if let Some(v) = value.as_str() {
                if key.eq_ignore_ascii_case(HTTP_OFFLOAD_BODY_HEADER) {
                    offload_body_to_native = header_truthy(v);
                    continue;
                }
                if key.eq_ignore_ascii_case(HTTP_WASI_TRANSFORM_HEADER) {
                    wasi_transform_plan = Some(parse_wasi_transform_plan(v)?);
                    continue;
                }
                builder = builder.header(&key, v);
            }
        }
    }

    if wasi_transform_plan.is_some() && !offload_body_to_native {
        return Err(anyhow!(
            "使用 wasi transform 时必须同时开启 {HTTP_OFFLOAD_BODY_HEADER}"
        ));
    }

    if let Some(content) = body {
        builder = builder.body(content);
    }

    let response = builder.send().await.context("发送 HTTP 请求失败")?;
    let status = response.status();
    let final_url = response.url().to_string();

    for (name, value) in response.headers() {
        let value_text = value
            .to_str()
            .context("解析 HTTP 响应头失败")?
            .to_string();
        headers_map.insert(name.to_string(), Value::String(value_text));
    }

    if offload_body_to_native {
        let mut body_bytes = response
            .bytes()
            .await
            .context("读取 HTTP 响应体字节失败")?
            .to_vec();

        let mut wasi_applied = false;
        let mut wasi_need_js_processing = false;
        let mut wasi_function: Option<String> = None;
        let mut wasi_output_type: Option<String> = None;
        if let Some(plan) = &wasi_transform_plan {
            wasi_need_js_processing = plan.js_process.unwrap_or(false);
            wasi_function = plan.function.clone();
            wasi_output_type = Some(plan.output_type.clone());
            body_bytes = run_wasi_transform_once(plan, body_bytes).await?;
            wasi_applied = true;
        }

        let native_buffer_id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
        let body_len = body_bytes.len();

        {
            let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
            pool.insert(native_buffer_id, body_bytes);
        }

        headers_map.insert(
            "x-rquickjs-host-offloaded".to_string(),
            Value::String("1".to_string()),
        );
        headers_map.insert(
            "x-rquickjs-host-native-buffer-id".to_string(),
            Value::String(native_buffer_id.to_string()),
        );

        return Ok(json!({
            "ok": true,
            "status": status.as_u16(),
            "statusText": status.canonical_reason().unwrap_or(""),
            "url": final_url,
            "headers": headers_map,
            "body": "",
            "offloaded": true,
            "nativeBufferId": native_buffer_id,
            "offloadedBytes": body_len,
            "wasiApplied": wasi_applied,
            "wasiNeedJsProcessing": wasi_need_js_processing,
            "wasiFunction": wasi_function,
            "wasiOutputType": wasi_output_type
        })
        .to_string());
    }

    let body_text = response.text().await.context("读取 HTTP 响应体失败")?;

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

fn parse_u8_json_array(data_json: &str) -> AnyResult<Vec<u8>> {
    let value: Value = serde_json::from_str(data_json).context("解析字节数组 JSON 失败")?;
    let arr = value
        .as_array()
        .ok_or_else(|| anyhow!("数据必须是字节数组"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let n = item
            .as_u64()
            .ok_or_else(|| anyhow!("字节数组元素必须是整数"))?;
        if n > 255 {
            return Err(anyhow!("字节数组元素必须在 0-255 范围"));
        }
        out.push(n as u8);
    }
    Ok(out)
}

pub fn native_buffer_put(data_json: String) -> String {
    let bytes = match parse_u8_json_array(&data_json) {
        Ok(bytes) => bytes,
        Err(error) => return json!({ "ok": false, "error": format!("{error}") }).to_string(),
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

fn parse_chain_steps(steps_json: &str) -> AnyResult<Vec<(String, Option<u64>)>> {
    let value: Value = serde_json::from_str(steps_json).context("解析 steps JSON 失败")?;
    let arr = value
        .as_array()
        .ok_or_else(|| anyhow!("steps 必须是数组"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let obj = item
            .as_object()
            .ok_or_else(|| anyhow!("steps 元素必须是对象"))?;
        let op = obj
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("steps 元素缺少 op 字段"))?
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
        Err(error) => return json!({ "ok": false, "error": format!("{error}") }).to_string(),
    };

    let output_id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    pool.insert(output_id, output);
    json!({ "ok": true, "id": output_id }).to_string()
}

pub fn native_exec_chain(input_id: u64, steps_json: String) -> String {
    let steps = match parse_chain_steps(&steps_json) {
        Ok(steps) => steps,
        Err(error) => return json!({ "ok": false, "error": format!("{error}") }).to_string(),
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
            Err(error) => return json!({ "ok": false, "error": format!("{error}") }).to_string(),
        };
    }

    let output_id = NATIVE_BUF_ID.fetch_add(1, Ordering::Relaxed);
    let mut pool = native_pool().lock().expect("native buffer 池加锁失败");
    pool.insert(output_id, current);
    json!({ "ok": true, "id": output_id }).to_string()
}

fn parse_argv(args_json: Option<String>) -> AnyResult<Vec<String>> {
    let Some(raw) = args_json else {
        return Ok(Vec::new());
    };
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&raw).context("解析 argv JSON 失败")?;
    let arr = value
        .as_array()
        .ok_or_else(|| anyhow!("argv 必须是字符串数组"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(
            item.as_str()
                .ok_or_else(|| anyhow!("argv 必须是字符串数组"))?
                .to_string(),
        );
    }
    Ok(out)
}

const WASI_MODULE_CACHE_MAX_ENTRIES: usize = 64;

fn wasi_module_get_or_compile(wasm_bytes: &[u8]) -> AnyResult<Module> {
    {
        let cache = wasi_module_cache().lock().expect("wasi module cache 加锁失败");
        if let Some(module) = cache.get(wasm_bytes) {
            WASI_CACHE_HITS.fetch_add(1, Ordering::Relaxed);
            return Ok(module.clone());
        }
    }

    WASI_CACHE_MISSES.fetch_add(1, Ordering::Relaxed);

    let engine = wasi_engine();
    let module =
        Module::new(engine, wasm_bytes).map_err(|e| anyhow!("编译 WASM 模块失败: {e}"))?;

    {
        let key = wasm_bytes.to_vec();
        let mut cache = wasi_module_cache().lock().expect("wasi module cache 加锁失败");
        if let Some(existing) = cache.get(wasm_bytes) {
            return Ok(existing.clone());
        }
        cache.insert(key.clone(), module.clone());

        let mut order = wasi_module_cache_order()
            .lock()
            .expect("wasi module cache order 加锁失败");
        order.push_back(key);

        while cache.len() > WASI_MODULE_CACHE_MAX_ENTRIES {
            if let Some(oldest) = order.pop_front() {
                cache.remove(&oldest);
                WASI_CACHE_EVICTIONS.fetch_add(1, Ordering::Relaxed);
            } else {
                break;
            }
        }
    }

    Ok(module)
}

fn wasi_run_inner(
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
        Err(error) => return json!({ "ok": false, "error": format!("{error}") }).to_string(),
    };

    let engine = wasi_engine();
    let module = match wasi_module_get_or_compile(&wasm_bytes) {
        Ok(module) => module,
        Err(error) => return json!({ "ok": false, "error": format!("{error}") }).to_string(),
    };

    let linker = match wasi_linker() {
        Ok(linker) => linker,
        Err(error) => return json!({ "ok": false, "error": format!("{error}") }).to_string(),
    };

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
    let mut store = Store::new(engine, wasi);

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

pub fn wasi_run_start(
    module_id: u64,
    stdin_id: Option<u64>,
    args_json: Option<String>,
    consume_module: bool,
) -> String {
    {
        let mut pool = wasi_req_pool().lock().expect("wasi 请求池加锁失败");
        cleanup_stale_pending(&mut pool, &WASI_STALE_DROPS);
        if pool.len() >= WASI_MAX_PENDING {
            return json!({ "ok": false, "error": "wasi pending 队列已满" }).to_string();
        }
    }

    let id = WASI_REQ_ID.fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::channel::<String>();
    let sem = Arc::clone(wasi_io_sem());

    let task = host_async_runtime().spawn(async move {
        let permit = match timeout(Duration::from_secs(15), sem.acquire_owned()).await {
            Ok(Ok(permit)) => permit,
            Ok(Err(_)) => {
                let _ = tx.send(json!({ "ok": false, "error": "wasi 并发控制器不可用" }).to_string());
                return;
            }
            Err(_) => {
                let _ = tx.send(json!({ "ok": false, "error": "wasi 等待并发许可超时" }).to_string());
                return;
            }
        };
        let payload = tokio::task::spawn_blocking(move || {
            wasi_run_inner(module_id, stdin_id, args_json, consume_module)
        })
        .await
        .unwrap_or_else(|e| json!({ "ok": false, "error": e.to_string() }).to_string());
        drop(permit);
        let _ = tx.send(payload);
    });

    {
        let mut pool = wasi_req_pool().lock().expect("wasi 请求池加锁失败");
        pool.insert(
            id,
            PendingTask {
                rx,
                task,
                created_at: Instant::now(),
            },
        );
    }

    json!({ "ok": true, "id": id }).to_string()
}

pub fn wasi_run_try_take(id: u64) -> String {
    let mut pool = wasi_req_pool().lock().expect("wasi 请求池加锁失败");
    cleanup_stale_pending(&mut pool, &WASI_STALE_DROPS);
    let Some(pending) = pool.get_mut(&id) else {
        return json!({ "ok": false, "error": "request id 不存在" }).to_string();
    };

    match pending.rx.try_recv() {
        Ok(result) => {
            pool.remove(&id);
            json!({ "ok": true, "done": true, "result": result }).to_string()
        }
        Err(TryRecvError::Empty) => json!({ "ok": true, "done": false }).to_string(),
        Err(TryRecvError::Disconnected) => {
            pool.remove(&id);
            json!({ "ok": false, "error": "wasi 执行任务异常退出" }).to_string()
        }
    }
}

pub fn wasi_run_drop(id: u64) -> String {
    let mut pool = wasi_req_pool().lock().expect("wasi 请求池加锁失败");
    let existed = if let Some(pending) = pool.remove(&id) {
        pending.task.abort();
        true
    } else {
        false
    };
    json!({ "ok": true, "dropped": existed }).to_string()
}

fn parse_host_ok_payload(raw: String) -> AnyResult<Value> {
    let payload: Value = serde_json::from_str(&raw).context("解析宿主返回 JSON 失败")?;
    if payload.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(payload)
    } else {
        Err(anyhow!(
            "{}",
            payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("调用失败")
        ))
    }
}

fn parse_bridge_args(args_json: Option<String>) -> AnyResult<Vec<Value>> {
    let Some(raw) = args_json else {
        return Ok(Vec::new());
    };
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = serde_json::from_str(&raw).context("解析 bridge args JSON 失败")?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| anyhow!("args 必须是数组"))
}

pub fn call_js_global_function(
    ctx: &Ctx<'_>,
    function_name: String,
    args_json: Option<String>,
) -> AnyResult<Value> {
    let args = parse_bridge_args(args_json)?;
    let function_name_json = serde_json::to_string(&function_name).context("序列化函数名失败")?;
    let args_literal = serde_json::to_string(&args).context("序列化函数参数失败")?;

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

    let promise: Promise = ctx.eval(script).context("执行 JS 调用脚本失败")?;
    let raw: String = promise.finish().context("等待 JS Promise 失败")?;
    let payload: Value = serde_json::from_str(&raw).context("解析 JS 返回 JSON 失败")?;
    if payload.get("ok").and_then(Value::as_bool) == Some(true) {
        Ok(payload.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(anyhow!(
            "{}",
            payload
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("调用失败")
        ))
    }
}

pub fn plugin_load(ctx: &Ctx<'_>, script: String) -> AnyResult<()> {
    ctx.eval::<(), _>(script).context("加载插件脚本失败")
}

pub fn plugin_get_info(ctx: &Ctx<'_>, name: String) -> AnyResult<Value> {
    call_js_global_function(
        ctx,
        "__plugin_host_get_info".to_string(),
        Some(json!([name]).to_string()),
    )
}

pub fn plugin_list(ctx: &Ctx<'_>) -> AnyResult<Value> {
    call_js_global_function(ctx, "__plugin_host_list".to_string(), None)
}

fn require_arg<'a>(args: &'a [Value], index: usize, name: &str) -> AnyResult<&'a Value> {
    args.get(index).ok_or_else(|| anyhow!("缺少参数: {name}"))
}

fn require_str_arg(args: &[Value], index: usize, name: &str) -> AnyResult<String> {
    require_arg(args, index, name)?
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("参数 {name} 必须是字符串"))
}

fn require_u64_arg(args: &[Value], index: usize, name: &str) -> AnyResult<u64> {
    require_arg(args, index, name)?
        .as_u64()
        .ok_or_else(|| anyhow!("参数 {name} 必须是非负整数"))
}

fn parse_u8_json_value(value: &Value) -> AnyResult<Vec<u8>> {
    let arr = value
        .as_array()
        .ok_or_else(|| anyhow!("数据必须是字节数组"))?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let n = item
            .as_u64()
            .ok_or_else(|| anyhow!("字节数组元素必须是整数"))?;
        if n > 255 {
            return Err(anyhow!("字节数组元素必须在 0-255 范围"));
        }
        out.push(n as u8);
    }
    Ok(out)
}

fn bridge_call_inner(name: String, args_json: Option<String>) -> AnyResult<Value> {
    let args = parse_bridge_args(args_json)?;

    match name.as_str() {
        "math.add" => {
            let a = require_arg(&args, 0, "a")?
                .as_f64()
                .ok_or_else(|| anyhow!("参数 a 必须是数字"))?;
            let b = require_arg(&args, 1, "b")?
                .as_f64()
                .ok_or_else(|| anyhow!("参数 b 必须是数字"))?;
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
                None => Err(anyhow!("buffer id 不存在")),
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
        _ => Err(anyhow!("不支持的 bridge 方法: {name}")),
    }
}

pub fn host_call(name: String, args_json: Option<String>) -> String {
    match bridge_call_inner(name, args_json) {
        Ok(data) => json!({ "ok": true, "data": data }).to_string(),
        Err(error) => json!({ "ok": false, "error": format!("{error:#}") }).to_string(),
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
        Err(error) => return json!({ "ok": false, "code": "EINVAL", "error": format!("{error}") }).to_string(),
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
