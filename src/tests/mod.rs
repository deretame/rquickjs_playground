pub mod axios;
pub mod compat;
pub mod fetch;
pub mod fs;
pub mod native;
pub mod runtime;
pub mod xhr;

pub use crate::web_runtime::{run_async_script, run_async_script_with_axios};
use serde_json::{json, Map, Value};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Method as TinyMethod, Response, Server};

static PNPM_CASES_BUILD: OnceLock<Result<(), String>> = OnceLock::new();

pub fn ensure_pnpm_cases_built() {
    let result = PNPM_CASES_BUILD.get_or_init(|| {
        let mut demo_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        demo_dir.push("pnpm_demo");

        let output = if cfg!(windows) {
            Command::new("cmd")
                .args(["/C", "pnpm run build:cases"])
                .current_dir(&demo_dir)
                .output()
                .map_err(|e| format!("执行 pnpm build:cases 失败: {e}"))?
        } else {
            Command::new("pnpm")
                .args(["run", "build:cases"])
                .current_dir(&demo_dir)
                .output()
                .map_err(|e| format!("执行 pnpm build:cases 失败: {e}"))?
        };

        if output.status.success() {
            Ok(())
        } else {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "pnpm build:cases 失败\nstdout:\n{stdout}\nstderr:\n{stderr}"
            ))
        }
    });

    if let Err(err) = result {
        panic!("{err}");
    }
}

pub fn spawn_test_server(limit: usize) -> (String, mpsc::Sender<()>, thread::JoinHandle<()>) {
    spawn_test_server_with_headers(limit, None)
}

pub fn spawn_test_server_with_headers(
    limit: usize,
    extra_headers: Option<Vec<(&'static str, &'static str)>>,
) -> (String, mpsc::Sender<()>, thread::JoinHandle<()>) {
    let server = Server::http("127.0.0.1:0").expect("启动测试服务失败");
    let addr = format!("http://{}", server.server_addr());
    let (tx, rx) = mpsc::channel::<()>();

    let handle = thread::spawn(move || {
        for _ in 0..limit {
            if rx.try_recv().is_ok() {
                break;
            }
            match server.recv_timeout(Duration::from_millis(100)) {
                Ok(Some(mut request)) => {
                    let method = match request.method() {
                        TinyMethod::Get => "GET",
                        TinyMethod::Post => "POST",
                        TinyMethod::Put => "PUT",
                        TinyMethod::Delete => "DELETE",
                        TinyMethod::Patch => "PATCH",
                        TinyMethod::Head => "HEAD",
                        TinyMethod::Options => "OPTIONS",
                        _ => "OTHER",
                    };

                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);

                    let mut headers = Map::new();
                    for header in request.headers() {
                        headers.insert(
                            header.field.as_str().to_string().to_lowercase(),
                            Value::String(header.value.as_str().to_string()),
                        );
                    }

                    let payload = json!({
                        "method": method,
                        "path": request.url(),
                        "body": body,
                        "headers": headers,
                    });

                    let mut resp_builder =
                        Response::from_string(payload.to_string()).with_status_code(200);

                    resp_builder = resp_builder
                        .with_header(
                            tiny_http::Header::from_bytes(
                                b"Content-Type".as_slice(),
                                b"application/json".as_slice(),
                            )
                            .expect("构造响应头失败"),
                        )
                        .with_header(
                            tiny_http::Header::from_bytes(
                                b"X-Custom".as_slice(),
                                b"custom-value".as_slice(),
                            )
                            .expect("构造自定义响应头失败"),
                        );

                    if let Some(ref extra) = extra_headers {
                        for (key, value) in extra {
                            resp_builder = resp_builder.with_header(
                                tiny_http::Header::from_bytes(key.as_bytes(), value.as_bytes())
                                    .expect("构造额外响应头失败"),
                            );
                        }
                    }

                    let resp = resp_builder;
                    let _ = request.respond(resp);
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }
    });

    (addr, tx, handle)
}

pub fn spawn_test_server_with_statuses(
    statuses: Vec<u16>,
) -> (String, mpsc::Sender<()>, thread::JoinHandle<()>) {
    let limit = statuses.len().max(1);
    let server = Server::http("127.0.0.1:0").expect("启动测试服务失败");
    let addr = format!("http://{}", server.server_addr());
    let (tx, rx) = mpsc::channel::<()>();
    let statuses = Arc::new(Mutex::new(VecDeque::from(statuses)));

    let handle = thread::spawn(move || {
        for _ in 0..limit {
            if rx.try_recv().is_ok() {
                break;
            }
            match server.recv_timeout(Duration::from_millis(100)) {
                Ok(Some(mut request)) => {
                    let method = match request.method() {
                        TinyMethod::Get => "GET",
                        TinyMethod::Post => "POST",
                        TinyMethod::Put => "PUT",
                        TinyMethod::Delete => "DELETE",
                        TinyMethod::Patch => "PATCH",
                        TinyMethod::Head => "HEAD",
                        TinyMethod::Options => "OPTIONS",
                        _ => "OTHER",
                    };

                    let mut body = String::new();
                    let _ = request.as_reader().read_to_string(&mut body);

                    let mut headers = Map::new();
                    for header in request.headers() {
                        headers.insert(
                            header.field.as_str().to_string().to_lowercase(),
                            Value::String(header.value.as_str().to_string()),
                        );
                    }

                    let status = {
                        let mut queue = statuses.lock().expect("状态队列加锁失败");
                        queue.pop_front().unwrap_or(200)
                    };

                    let payload = json!({
                        "method": method,
                        "path": request.url(),
                        "body": body,
                        "headers": headers,
                        "status": status,
                    });

                    let resp = Response::from_string(payload.to_string())
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_bytes(
                                b"Content-Type".as_slice(),
                                b"application/json".as_slice(),
                            )
                            .expect("构造响应头失败"),
                        )
                        .with_header(
                            tiny_http::Header::from_bytes(
                                b"X-Custom".as_slice(),
                                b"custom-value".as_slice(),
                            )
                            .expect("构造自定义响应头失败"),
                        );

                    let _ = request.respond(resp);
                }
                Ok(None) => {}
                Err(_) => {}
            }
        }
    });

    (addr, tx, handle)
}
