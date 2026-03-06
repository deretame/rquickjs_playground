use crate::tests::{
    ensure_pnpm_cases_built, run_async_script, run_async_script_with_axios, spawn_test_server,
};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn case_bundle_path(name: &str) -> PathBuf {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("pnpm_demo");
    p.push("dist");
    p.push("cases");
    p.push(format!("{name}.js"));
    p
}

fn run_case(name: &str, config: Value, with_axios: bool) -> Value {
    ensure_pnpm_cases_built();
    let bundle = fs::read_to_string(case_bundle_path(name)).expect("读取 case bundle 失败");
    let bundle_json = serde_json::to_string(&bundle).expect("序列化 bundle 失败");
    let config_json = serde_json::to_string(&config).expect("序列化 config 失败");

    let script = format!(
        r#"
      (async () => {{
        try {{
          const code = {bundle_json};
          const cfg = {config_json};
          eval(code);
          const out = await globalThis.__caseMain(cfg);
          return JSON.stringify(out);
        }} catch (err) {{
          return JSON.stringify({{
            ok: false,
            __error: String(err && (err.stack || err.message) ? (err.stack || err.message) : err)
          }});
        }}
      }})()
    "#
    );

    let result = if with_axios {
        run_async_script_with_axios(&script)
    } else {
        run_async_script(&script)
    }
    .expect("执行 bundle case 失败");

    serde_json::from_str(&result).expect("解析 case 结果失败")
}

fn assert_case_ok(out: &Value) {
    if out["ok"] != true {
        panic!(
            "case 执行失败: {}",
            out["__error"].as_str().unwrap_or("未知错误")
        );
    }
}

fn unique_temp_dir() -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系统时间异常")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rquickjs-case-{ts}"));
    fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

#[test]
fn compiled_fetch_case_runs() {
    ensure_pnpm_cases_built();
    let (base_url, tx, handle) = spawn_test_server(4);
    let out = run_case("fetch", serde_json::json!({ "baseUrl": base_url }), false);
    assert_case_ok(&out);
    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn compiled_xhr_case_runs() {
    ensure_pnpm_cases_built();
    let (base_url, tx, handle) = spawn_test_server(4);
    let out = run_case("xhr", serde_json::json!({ "baseUrl": base_url }), false);
    assert_case_ok(&out);
    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn compiled_axios_case_runs() {
    ensure_pnpm_cases_built();
    let (base_url, tx, handle) = spawn_test_server(1);
    let out = run_case("axios", serde_json::json!({ "baseUrl": base_url }), false);
    assert_case_ok(&out);
    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn compiled_fs_case_runs() {
    let dir = unique_temp_dir();
    let base_dir = dir.to_string_lossy().replace('\\', "/");
    let out = run_case("fs", serde_json::json!({ "baseDir": base_dir }), false);
    assert_case_ok(&out);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn compiled_native_case_runs() {
    let out = run_case("native", serde_json::json!({}), false);
    assert_case_ok(&out);
}

#[test]
fn compiled_runtime_case_runs() {
    let out = run_case("runtime", serde_json::json!({}), false);
    assert_case_ok(&out);
}

#[test]
fn compiled_wasi_case_runs() {
    let out = run_case("wasi", serde_json::json!({}), false);
    assert_case_ok(&out);
}

#[test]
fn compiled_cache_case_runs() {
    let out = run_case("cache", serde_json::json!({}), false);
    assert_case_ok(&out);
}

#[test]
fn compiled_bridge_case_runs() {
    let out = run_case("bridge", serde_json::json!({}), false);
    assert_case_ok(&out);
}
