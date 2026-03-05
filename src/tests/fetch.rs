use crate::tests::{run_async_script, spawn_test_server};
use serde_json::Value;

#[test]
fn fetch_get_json() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await fetch("{}/hello?from=test");
            const data = await res.json();
            return JSON.stringify({{
              status: res.status,
              method: data.method,
              path: data.path
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "GET");
    assert_eq!(parsed["path"], "/hello?from=test");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn fetch_post_json_body() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await fetch("{}/echo", {{
              method: "POST",
              headers: {{ "x-from": "fetch-test" }},
              body: {{ name: "quickjs" }}
            }});
            const data = await res.json();
            return JSON.stringify({{
              method: data.method,
              body: data.body,
              header: data.headers["x-from"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["method"], "POST");
    assert_eq!(parsed["body"], "{\"name\":\"quickjs\"}");
    assert_eq!(parsed["header"], "fetch-test");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn fetch_headers() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await fetch("{}/headers");
            const hasContentType = res.headers.has("content-type");
            const contentType = res.headers.get("content-type");
            return JSON.stringify({{
              status: res.status,
              hasContentType,
              contentType: contentType
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["hasContentType"], true);
    assert!(parsed["contentType"]
        .as_str()
        .unwrap_or("")
        .contains("application/json"));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn fetch_put_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await fetch("{}/fetch-put", {{
              method: "PUT",
              body: JSON.stringify({{ name: "test" }})
            }});
            const data = await res.json();
            return JSON.stringify({{
              status: res.status,
              method: data.method,
              body: data.body
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "PUT");
    assert_eq!(parsed["body"], "{\"name\":\"test\"}");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn fetch_delete_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await fetch("{}/fetch-delete/123", {{
              method: "DELETE"
            }});
            const data = await res.json();
            return JSON.stringify({{
              status: res.status,
              method: data.method,
              path: data.path
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "DELETE");
    assert_eq!(parsed["path"], "/fetch-delete/123");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn fetch_patch_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await fetch("{}/fetch-patch", {{
              method: "PATCH",
              body: JSON.stringify({{ name: "updated" }})
            }});
            const data = await res.json();
            return JSON.stringify({{
              status: res.status,
              method: data.method,
              body: data.body
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "PATCH");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn fetch_text_response() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await fetch("{}/text");
            const text = await res.text();
            return JSON.stringify({{
              status: res.status,
              isObject: text.includes("method")
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["isObject"], true);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn fetch_abort_controller() {
    let script = r#"
      (async () => {
        const controller = new AbortController();
        controller.abort("cancelled");
        try {
          await fetch("http://127.0.0.1:9/unreachable", { signal: controller.signal });
          return "unexpected";
        } catch (err) {
          return `${err.name}:${String(err.message || "")}`;
        }
      })()
    "#;

    let result = run_async_script(script).expect("执行脚本失败");
    assert!(result.starts_with("AbortError:"));
}

#[test]
fn fetch_multiple_headers() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await fetch("{}/multi-headers", {{
              headers: {{
                "X-Header-1": "value1",
                "X-Header-2": "value2",
                "X-Header-3": "value3"
              }}
            }});
            const data = await res.json();
            return JSON.stringify({{
              status: res.status,
              h1: data.headers["x-header-1"],
              h2: data.headers["x-header-2"],
              h3: data.headers["x-header-3"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["h1"], "value1");
    assert_eq!(parsed["h2"], "value2");
    assert_eq!(parsed["h3"], "value3");

    let _ = tx.send(());
    let _ = handle.join();
}
