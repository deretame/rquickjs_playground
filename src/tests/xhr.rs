use crate::tests::{run_async_script, spawn_test_server};
use serde_json::Value;
use wat::parse_str;

fn wasi_echo_stdin_module_bytes() -> Vec<u8> {
    parse_str(
        r#"
        (module
          (import "wasi_snapshot_preview1" "fd_read"
            (func $fd_read (param i32 i32 i32 i32) (result i32)))
          (import "wasi_snapshot_preview1" "fd_write"
            (func $fd_write (param i32 i32 i32 i32) (result i32)))

          (memory (export "memory") 1)

          (func (export "_start")
            i32.const 0
            i32.const 100
            i32.store
            i32.const 4
            i32.const 4096
            i32.store

            i32.const 0
            i32.const 0
            i32.const 1
            i32.const 8
            call $fd_read
            drop

            i32.const 16
            i32.const 100
            i32.store
            i32.const 20
            i32.const 8
            i32.load
            i32.store

            i32.const 1
            i32.const 16
            i32.const 1
            i32.const 24
            call $fd_write
            drop
          )
        )
        "#,
    )
    .expect("构建 wasi echo 模块失败")
}

#[test]
fn xhr_get_works() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("GET", "{}/xhr?ok=1", true);
              xhr.onload = () => resolve(JSON.stringify({{
                status: xhr.status,
                body: JSON.parse(xhr.responseText)
              }}));
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["body"]["path"], "/xhr?ok=1");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_post_with_body() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("POST", "{}/xhr-post", true);
              xhr.setRequestHeader("Content-Type", "application/json");
              xhr.onload = () => resolve(JSON.stringify({{
                status: xhr.status,
                body: JSON.parse(xhr.responseText)
              }}));
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send(JSON.stringify({{ name: "test", value: 123 }}));
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["body"]["method"], "POST");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_response_headers() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("GET", "{}/xhr-headers", true);
              xhr.onload = () => resolve(JSON.stringify({{
                status: xhr.status,
                contentType: xhr.getResponseHeader("content-type"),
                customHeader: xhr.getResponseHeader("x-custom"),
                allHeaders: xhr.getAllResponseHeaders()
              }}));
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert!(parsed["contentType"]
        .as_str()
        .unwrap_or("")
        .contains("application/json"));
    assert_eq!(parsed["customHeader"], "custom-value");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_put_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("PUT", "{}/xhr-put", true);
              xhr.setRequestHeader("Content-Type", "application/json");
              xhr.onload = () => resolve(JSON.stringify({{
                status: xhr.status,
                method: JSON.parse(xhr.responseText).method
              }}));
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send(JSON.stringify({{ id: 1, name: "updated" }}));
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "PUT");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_delete_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("DELETE", "{}/xhr-delete/456", true);
              xhr.onload = () => resolve(JSON.stringify({{
                status: xhr.status,
                path: JSON.parse(xhr.responseText).path
              }}));
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["path"], "/xhr-delete/456");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_ready_state() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const states = [];
            const xhr = new XMLHttpRequest();
            xhr.open("GET", "{}/xhr-state", true);
            states.push({{ state: xhr.readyState, name: "after-open" }});
            xhr.onreadystatechange = () => {{
              states.push({{ state: xhr.readyState, name: "onchange" }});
            }};
            xhr.onload = () => {{
              states.push({{ state: xhr.readyState, name: "onload", status: xhr.status }});
            }};
            await new Promise((resolve) => {{
              xhr.onloadend = () => resolve(null);
              xhr.send();
            }});
            return JSON.stringify({{
              status: xhr.status,
              states: states
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    let states = parsed["states"].as_array().unwrap();
    assert!(states
        .iter()
        .any(|s| s["name"] == "after-open" && s["state"] == 1));
    assert!(states
        .iter()
        .any(|s| s["name"] == "onload" && s["state"] == 4));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_timeout() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("GET", "{}/xhr-timeout", true);
              xhr.timeout = 5000;
              xhr.ontimeout = () => resolve(JSON.stringify({{
                timedOut: true,
                status: xhr.status
              }}));
              xhr.onload = () => resolve(JSON.stringify({{
                timedOut: false,
                status: xhr.status
              }}));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["timedOut"], false);
    assert_eq!(parsed["status"], 200);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_multiple_headers() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("GET", "{}/xhr-multi", true);
              xhr.setRequestHeader("X-Custom-1", "value1");
              xhr.setRequestHeader("X-Custom-2", "value2");
              xhr.setRequestHeader("X-Custom-3", "value3");
              xhr.onload = () => resolve(JSON.stringify({{
                status: xhr.status,
                headers: JSON.parse(xhr.responseText).headers
              }}));
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["headers"]["x-custom-1"], "value1");
    assert_eq!(parsed["headers"]["x-custom-2"], "value2");
    assert_eq!(parsed["headers"]["x-custom-3"], "value3");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_offload_binary_to_native_buffer() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("GET", "{}/xhr-offload", true);
              xhr.setRequestHeader("x-rquickjs-host-offload-binary-v1", "true");
              xhr.onload = async () => {{
                try {{
                  const id = Number(xhr.nativeBufferId || 0);
                  const bytes = await native.take(id);
                  const text = new TextDecoder().decode(bytes);
                  resolve(JSON.stringify({{
                    status: xhr.status,
                    offloaded: xhr.offloaded === true,
                    offloadedBytes: xhr.offloadedBytes,
                    responseText: xhr.responseText,
                    hasPayload: text.includes("\"method\":\"GET\"")
                  }}));
                }} catch (err) {{
                  reject(err);
                }}
              }};
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["offloaded"], true);
    assert_eq!(parsed["responseText"], "");
    assert!(parsed["offloadedBytes"].as_u64().unwrap_or(0) > 0);
    assert_eq!(parsed["hasPayload"], true);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_offload_with_wasi_transform_success() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let wasm = wasi_echo_stdin_module_bytes();
    let wasm_json = serde_json::to_string(&wasm).expect("序列化 wasm 失败");

    let script = format!(
        r#"
          (async () => {{
            const wasm = new Uint8Array({wasm_json});
            const moduleId = await native.put(wasm);
            const planJson = JSON.stringify({{
              moduleId,
              function: "echo",
              args: {{ mode: "passthrough" }},
              jsProcess: true,
              outputType: "binary"
            }});
            const planBytes = new TextEncoder().encode(planJson);
            let bin = "";
            for (let i = 0; i < planBytes.length; i += 1) bin += String.fromCharCode(planBytes[i]);
            const plan = btoa(bin);

            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("GET", "{}/xhr-offload-wasi", true);
              xhr.setRequestHeader("x-rquickjs-host-offload-binary-v1", "true");
              xhr.setRequestHeader("x-rquickjs-host-wasi-transform-b64-v1", plan);
              xhr.onload = async () => {{
                try {{
                  const bytes = await native.take(Number(xhr.nativeBufferId || 0));
                  const text = new TextDecoder().decode(bytes);
                  resolve(JSON.stringify({{
                    status: xhr.status,
                    offloaded: xhr.offloaded === true,
                    wasiApplied: xhr.wasiApplied === true,
                    wasiNeedJsProcessing: xhr.wasiNeedJsProcessing === true,
                    wasiFunction: xhr.wasiFunction,
                    wasiOutputType: xhr.wasiOutputType,
                    hasPayload: text.includes("\"method\":\"GET\"")
                  }}));
                }} catch (err) {{
                  reject(err);
                }}
              }};
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["offloaded"], true);
    assert_eq!(parsed["wasiApplied"], true);
    assert_eq!(parsed["wasiNeedJsProcessing"], true);
    assert_eq!(parsed["wasiFunction"], "echo");
    assert_eq!(parsed["wasiOutputType"], "binary");
    assert_eq!(parsed["hasPayload"], true);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_offload_with_wasi_transform_failure_errors() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const planJson = JSON.stringify({{
              moduleId: 999999,
              function: "echo",
              args: {{ mode: "passthrough" }},
              jsProcess: false,
              outputType: "binary"
            }});
            const planBytes = new TextEncoder().encode(planJson);
            let bin = "";
            for (let i = 0; i < planBytes.length; i += 1) bin += String.fromCharCode(planBytes[i]);
            const plan = btoa(bin);

            const out = await new Promise((resolve) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("GET", "{}/xhr-offload-wasi-fail", true);
              xhr.setRequestHeader("x-rquickjs-host-offload-binary-v1", "true");
              xhr.setRequestHeader("x-rquickjs-host-wasi-transform-b64-v1", plan);
              xhr.onload = () => resolve(JSON.stringify({{ ok: false, status: xhr.status }}));
              xhr.onerror = () => resolve(JSON.stringify({{ ok: true, status: xhr.status }}));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    assert_eq!(parsed["ok"], true);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_events() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const events = [];
            const xhr = new XMLHttpRequest();
            xhr.addEventListener("loadstart", () => events.push("loadstart"));
            xhr.addEventListener("load", () => events.push("load"));
            xhr.addEventListener("loadend", () => events.push("loadend"));
            xhr.open("GET", "{}/xhr-events", true);
            xhr.onload = () => events.push("onload");
            await new Promise((resolve) => {{
              xhr.onloadend = () => resolve(null);
              xhr.send();
            }});
            return JSON.stringify({{
              status: xhr.status,
              events: events
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    let events = parsed["events"].as_array().unwrap();
    assert!(
        events.len() >= 2,
        "Expected at least 2 events, got {:?}",
        events
    );

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn xhr_response_type_text() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const out = await new Promise((resolve, reject) => {{
              const xhr = new XMLHttpRequest();
              xhr.open("GET", "{}/xhr-resp-type", true);
              xhr.responseType = "text";
              xhr.onload = () => resolve(JSON.stringify({{
                status: xhr.status,
                responseType: xhr.responseType,
                isString: typeof xhr.response === "string"
              }}));
              xhr.onerror = () => reject(new Error("xhr error"));
              xhr.send();
            }});
            return out;
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["isString"], true);

    let _ = tx.send(());
    let _ = handle.join();
}
