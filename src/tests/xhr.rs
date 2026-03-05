use crate::tests::{run_async_script, spawn_test_server};
use serde_json::Value;

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
            xhr.send();
            await new Promise(r => setTimeout(r, 50));
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
            xhr.send();
            await new Promise(r => setTimeout(r, 50));
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
