use crate::tests::{
    run_async_script_with_axios, spawn_test_server, spawn_test_server_with_statuses,
};
use serde_json::Value;

#[test]
fn axios_get_works() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/axios-get?x=1", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              path: res.data.path,
              method: res.data.method
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["path"], "/axios-get?x=1");
    assert_eq!(parsed["method"], "GET");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_post_json_and_headers() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.post(
              "{}/axios-post",
              {{ lib: "axios", runtime: "quickjs" }},
              {{
                adapter: "xhr",
                headers: {{ "x-axios": "yes" }}
              }}
            );
            return JSON.stringify({{
              status: res.status,
              method: res.data.method,
              body: res.data.body,
              header: res.data.headers["x-axios"],
              contentType: res.data.headers["content-type"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "POST");
    assert_eq!(parsed["header"], "yes");
    assert_eq!(
        parsed["body"],
        "{\"lib\":\"axios\",\"runtime\":\"quickjs\"}"
    );
    assert!(parsed["contentType"]
        .as_str()
        .unwrap_or("")
        .contains("application/json"));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_put_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.put("{}/axios-put", {{ id: 1, name: "test" }}, {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              method: res.data.method,
              body: res.data.body
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "PUT");
    assert_eq!(parsed["body"], "{\"id\":1,\"name\":\"test\"}");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_delete_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.delete("{}/axios-delete/123", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              method: res.data.method,
              path: res.data.path
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "DELETE");
    assert_eq!(parsed["path"], "/axios-delete/123");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_patch_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.patch("{}/axios-patch", {{ name: "updated" }}, {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              method: res.data.method,
              body: res.data.body
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "PATCH");
    assert_eq!(parsed["body"], "{\"name\":\"updated\"}");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_head_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.head("{}/axios-head", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              method: "HEAD"
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "HEAD");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_options_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.options("{}/axios-options", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              method: res.data.method
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "OPTIONS");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_request_with_params() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/axios-params", {{
              adapter: "xhr",
              params: {{ page: 1, size: 10, filter: "test" }}
            }});
            return JSON.stringify({{
              status: res.status,
              path: res.data.path
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert!(parsed["path"].as_str().unwrap_or("").contains("page=1"));
    assert!(parsed["path"].as_str().unwrap_or("").contains("size=10"));
    assert!(parsed["path"]
        .as_str()
        .unwrap_or("")
        .contains("filter=test"));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_response_headers() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/axios-headers", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              contentType: res.headers["content-type"],
              customHeader: res.headers["x-custom"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
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
fn axios_request_interceptor() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            axios.interceptors.request.use((config) => {{
              config.headers["x-request-id"] = "req-12345";
              return config;
            }});
            const res = await axios.get("{}/axios-interceptor", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              requestId: res.data.headers["x-request-id"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["requestId"], "req-12345");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_response_interceptor() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            axios.interceptors.response.use((response) => {{
              response.data.extra = "added by interceptor";
              return response;
            }});
            const res = await axios.get("{}/axios-resp-interceptor", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              extra: res.data.extra
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["extra"], "added by interceptor");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_interceptor_error_handling() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            axios.interceptors.request.use((config) => {{
              throw new Error("Request blocked by interceptor");
            }});
            try {{
              await axios.get("{}/axios-error", {{ adapter: "xhr" }});
              return JSON.stringify({{ success: false }});
            }} catch (err) {{
              return JSON.stringify({{
                success: true,
                message: err.message
              }});
            }}
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["success"], true);
    assert!(parsed["message"]
        .as_str()
        .unwrap_or("")
        .contains("Request blocked"));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_interceptor_remove() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const interceptorId = axios.interceptors.request.use((config) => {{
              config.headers["x-added"] = "interceptor";
              return config;
            }});
            axios.interceptors.request.eject(interceptorId);
            const res = await axios.get("{}/interceptor-remove", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              added: res.data.headers["x-added"] || "not-set"
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["added"], "not-set");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_interceptor_chain() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            axios.interceptors.request.use((config) => {{
              config.headers["x-first"] = "1";
              return config;
            }}, (err) => Promise.reject(err));
            axios.interceptors.request.use((config) => {{
              config.headers["x-second"] = "2";
              return config;
            }});
            const res = await axios.get("{}/interceptor-chain", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              first: res.data.headers["x-first"],
              second: res.data.headers["x-second"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["first"], "1");
    assert_eq!(parsed["second"], "2");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_create_instance() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const api = axios.create({{
              baseURL: "{}/api",
              adapter: "xhr",
              timeout: 5000
            }});
            const res = await api.get("/users");
            return JSON.stringify({{
              status: res.status,
              path: res.data.path
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["path"], "/api/users");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_instance_interceptors() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const api = axios.create({{ adapter: "xhr", baseURL: "{}/" }});
            api.interceptors.request.use((config) => {{
              config.headers["x-api-key"] = "secret-key";
              return config;
            }});
            const res = await api.get("/instance-interceptor");
            return JSON.stringify({{
              status: res.status,
              apiKey: res.data.headers["x-api-key"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["apiKey"], "secret-key");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_default_config() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            axios.defaults.baseURL = "{}/default";
            axios.defaults.headers.common["Authorization"] = "Bearer token123";
            const res = await axios.get("/test", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              path: res.data.path,
              auth: res.data.headers["authorization"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["path"], "/default/test");
    assert_eq!(parsed["auth"], "Bearer token123");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_all_spread() {
    let (base_url, tx, handle) = spawn_test_server(4);
    let script = format!(
        r#"
          (async () => {{
            const [res1, res2] = await axios.all([
              axios.get("{}/axios-all-1", {{ adapter: "xhr" }}),
              axios.get("{}/axios-all-2", {{ adapter: "xhr" }})
            ]);
            return JSON.stringify({{
              status1: res1.status,
              path1: res1.data.path,
              status2: res2.status,
              path2: res2.data.path
            }});
          }})()
        "#,
        base_url, base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status1"], 200);
    assert_eq!(parsed["path1"], "/axios-all-1");
    assert_eq!(parsed["status2"], 200);
    assert_eq!(parsed["path2"], "/axios-all-2");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_spread() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.all([
              axios.get("{}/spread-1", {{ adapter: "xhr" }}),
              axios.get("{}/spread-2", {{ adapter: "xhr" }})
            ]);
            const result = axios.spread((a, b) => ({{
              first: a.data.path,
              second: b.data.path
            }}))(res);
            return JSON.stringify(result);
          }})()
        "#,
        base_url, base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["first"], "/spread-1");
    assert_eq!(parsed["second"], "/spread-2");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_request_config() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios({{
              method: "post",
              url: "{}/axios-config",
              data: {{ username: "admin", password: "123456" }},
              adapter: "xhr",
              headers: {{ "Content-Type": "application/x-www-form-urlencoded" }}
            }});
            return JSON.stringify({{
              status: res.status,
              method: res.data.method,
              contentType: res.data.headers["content-type"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "POST");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_response_status_text() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/status-ok", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              statusText: res.statusText
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["statusText"], "OK");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_is_cancel() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = r#"
      (async () => {
        const cancelToken = axios.CancelToken.source();
        cancelToken.cancel("manual cancel");
        try {
          await axios.get("{base_url}/cancel-test", {
            adapter: "xhr",
            cancelToken: cancelToken.token
          });
          return JSON.stringify({ isCancel: false });
        } catch (err) {
          return JSON.stringify({
            isCancel: axios.isCancel(err),
            message: err.message
          });
        }
      })()
    "#
    .replace("{base_url}", &base_url);

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["isCancel"], true);
    assert!(parsed["message"].as_str().unwrap_or("").contains("cancel"));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_is_axios_error() {
    let script = r#"
      (async () => {
        try {
          await axios.get("http://127.0.0.1:99999/no-such-server", {
            adapter: "xhr",
            timeout: 100
          });
          return JSON.stringify({ isError: false });
        } catch (err) {
          return JSON.stringify({
            isAxiosError: axios.isAxiosError(err),
            hasMessage: !!err.message
          });
        }
      })()
    "#;

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["isAxiosError"], true);
    assert_eq!(parsed["hasMessage"], true);
}

#[test]
fn axios_retry_request() {
    let (base_url, tx, handle) = spawn_test_server_with_statuses(vec![429, 429, 200]);
    let script = format!(
        r#"
          (async () => {{
            let attempt = 0;
            axios.interceptors.response.use(null, async (error) => {{
              if (error.response && error.response.status === 429 && attempt < 2) {{
                attempt++;
                return axios(error.config);
              }}
              throw error;
            }});
            const res = await axios.get("{}/retry", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              path: res.data.path,
              attempt: attempt
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["path"], "/retry");
    assert_eq!(parsed["attempt"], 2);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_response_config() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/axios-config", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              configUrl: res.config.url,
              configMethod: res.config.method,
              configAdapter: res.config.adapter
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["configMethod"], "get");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_get_without_body() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/no-body", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              hasData: !!res.data
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["hasData"], true);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_custom_timeout() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/timeout-test", {{ adapter: "xhr", timeout: 30000 }});
            return JSON.stringify({{
              status: res.status,
              path: res.data.path
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["path"], "/timeout-test");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_with_credentials() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/credentials", {{ 
              adapter: "xhr",
              withCredentials: true 
            }});
            return JSON.stringify({{
              status: res.status
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_response_type() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/response-type", {{ 
              adapter: "xhr",
              responseType: "text"
            }});
            return JSON.stringify({{
              status: res.status,
              isString: typeof res.data === "string"
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["isString"], true);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_multiple_interceptors() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const ids = [];
            ids.push(axios.interceptors.request.use((config) => {{
              config.headers["x-interceptor-1"] = "1";
              return config;
            }}));
            ids.push(axios.interceptors.request.use((config) => {{
              config.headers["x-interceptor-2"] = "2";
              return config;
            }}));
            ids.push(axios.interceptors.request.use((config) => {{
              config.headers["x-interceptor-3"] = "3";
              return config;
            }}));
            axios.interceptors.request.eject(ids[1]);
            const res = await axios.get("{}/multi-interceptor", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              i1: res.data.headers["x-interceptor-1"],
              i2: res.data.headers["x-interceptor-2"] || "removed",
              i3: res.data.headers["x-interceptor-3"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["i1"], "1");
    assert_eq!(parsed["i2"], "removed");
    assert_eq!(parsed["i3"], "3");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_response_interceptor_error() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            axios.interceptors.response.use((response) => {{
              throw new Error("Response interceptor error");
            }});
            try {{
              await axios.get("{}/resp-err", {{ adapter: "xhr" }});
              return JSON.stringify({{ success: false }});
            }} catch (err) {{
              return JSON.stringify({{
                success: true,
                message: err.message
              }});
            }}
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["success"], true);
    assert!(parsed["message"]
        .as_str()
        .unwrap_or("")
        .contains("Response interceptor"));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_transform_request() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.post("{}/transform", {{ value: 42 }}, {{
              adapter: "xhr",
              transformRequest: [(data) => {{
                return JSON.stringify({{ transformed: true, ...data }});
              }}]
            }});
            return JSON.stringify({{
              status: res.status,
              body: res.data.body
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");
    let transformed_body: Value =
        serde_json::from_str(parsed["body"].as_str().unwrap_or("{}")).expect("解析请求体失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(transformed_body["transformed"], true);
    assert_eq!(transformed_body["value"], 42);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_transform_response() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.get("{}/transform-resp", {{
              adapter: "xhr",
              transformResponse: [(data) => {{
                const parsed = JSON.parse(data);
                parsed.transformed = true;
                return parsed;
              }}]
            }});
            return JSON.stringify({{
              status: res.status,
              transformed: res.data.transformed,
              method: res.data.method
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["transformed"], true);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_base_url_inheritance() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const api = axios.create({{
              baseURL: "{}/api/v1",
              adapter: "xhr"
            }});
            const res = await api.get("/users/1");
            return JSON.stringify({{
              status: res.status,
              path: res.data.path
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["path"], "/api/v1/users/1");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_merge_config() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            axios.defaults.headers.common["X-Default"] = "default-value";
            const res = await axios.get("{}/merge-config", {{
              adapter: "xhr",
              headers: {{ "X-Request": "request-value" }}
            }});
            return JSON.stringify({{
              status: res.status,
              defaultHeader: res.data.headers["x-default"],
              requestHeader: res.data.headers["x-request"]
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["defaultHeader"], "default-value");
    assert_eq!(parsed["requestHeader"], "request-value");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_empty_data_post() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.post("{}/empty-data", "", {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              method: res.data.method,
              bodyLength: res.data.body.length
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "POST");

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_cancel_token_source() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const source = axios.CancelToken.source();
            source.cancel("Operation cancelled by user");
            try {{
              await axios.get("{}/cancel-source", {{ 
                adapter: "xhr",
                cancelToken: source.token 
              }});
              return JSON.stringify({{ success: false }});
            }} catch (err) {{
              return JSON.stringify({{
                success: axios.isCancel(err),
                message: err.message
              }});
            }}
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["success"], true);
    assert!(parsed["message"]
        .as_str()
        .unwrap_or("")
        .contains("cancelled"));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_promise_all_with_map() {
    let (base_url, tx, handle) = spawn_test_server(4);
    let script = format!(
        r#"
          (async () => {{
            const requests = [1, 2, 3].map(id => axios.get("{}/map/" + id, {{ adapter: "xhr" }}));
            const responses = await axios.all(requests);
            return JSON.stringify({{
              count: responses.length,
              paths: responses.map(r => r.data.path)
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["count"], 3);
    assert!(parsed["paths"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str().unwrap_or("").contains("/map/1")));

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_concurrent_requests() {
    let (base_url, tx, handle) = spawn_test_server(4);
    let script = format!(
        r#"
          (async () => {{
            const [res1, res2, res3] = await Promise.all([
              axios.get("{}/concurrent1", {{ adapter: "xhr" }}),
              axios.get("{}/concurrent2", {{ adapter: "xhr" }}),
              axios.get("{}/concurrent3", {{ adapter: "xhr" }})
            ]);
            return JSON.stringify({{
              status1: res1.status,
              status2: res2.status,
              status3: res3.status,
              path1: res1.data.path,
              path2: res2.data.path,
              path3: res3.data.path
            }});
          }})()
        "#,
        base_url, base_url, base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status1"], 200);
    assert_eq!(parsed["status2"], 200);
    assert_eq!(parsed["status3"], 200);

    let _ = tx.send(());
    let _ = handle.join();
}

#[test]
fn axios_json_data() {
    let (base_url, tx, handle) = spawn_test_server(2);
    let script = format!(
        r#"
          (async () => {{
            const res = await axios.post("{}/json", {{
              nested: {{ key: "value" }},
              array: [1, 2, 3],
              number: 42,
              bool: true,
              nullVal: null
            }}, {{ adapter: "xhr" }});
            return JSON.stringify({{
              status: res.status,
              method: res.data.method
            }});
          }})()
        "#,
        base_url
    );

    let result = run_async_script_with_axios(&script).expect("执行脚本失败");
    let parsed: Value = serde_json::from_str(&result).expect("解析结果失败");

    assert_eq!(parsed["status"], 200);
    assert_eq!(parsed["method"], "POST");

    let _ = tx.send(());
    let _ = handle.join();
}
