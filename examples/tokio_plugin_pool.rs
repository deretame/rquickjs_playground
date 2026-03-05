use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use rquickjs_playground::HostRuntime;
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[derive(Debug, Clone)]
struct InvokeItem {
    item_id: u64,
    plugin_name: String,
    function: String,
    args: Value,
}

#[derive(Debug, Clone)]
struct PluginRequest {
    request_id: String,
    items: Vec<InvokeItem>,
}

#[derive(Debug)]
enum ResultEvent {
    Item {
        request_id: String,
        item_id: u64,
        ok: bool,
        data: Option<Value>,
        error: Option<String>,
    },
    Done {
        request_id: String,
        total: usize,
    },
}

struct WorkerJob {
    request_id: String,
    item: InvokeItem,
    pending: Arc<AtomicUsize>,
    total: usize,
    response_tx: UnboundedSender<ResultEvent>,
}

struct PluginManager {
    workers: Vec<mpsc::Sender<WorkerJob>>,
}

impl PluginManager {
    fn new(worker_count: usize) -> Self {
        let mut workers = Vec::with_capacity(worker_count);

        for worker_id in 0..worker_count {
            let (tx, rx) = mpsc::channel::<WorkerJob>();
            workers.push(tx);

            thread::spawn(move || {
                let host = HostRuntime::new(false).expect("创建 HostRuntime 失败");
                host.eval_async(plugin_bootstrap_script())
                    .expect("初始化插件脚本失败");

                while let Ok(job) = rx.recv() {
                    let result = invoke_one(&host, &job.item);

                    match result {
                        Ok(data) => {
                            let _ = job.response_tx.send(ResultEvent::Item {
                                request_id: job.request_id.clone(),
                                item_id: job.item.item_id,
                                ok: true,
                                data: Some(data),
                                error: None,
                            });
                        }
                        Err(error) => {
                            let _ = job.response_tx.send(ResultEvent::Item {
                                request_id: job.request_id.clone(),
                                item_id: job.item.item_id,
                                ok: false,
                                data: None,
                                error: Some(error),
                            });
                        }
                    }

                    if job.pending.fetch_sub(1, Ordering::AcqRel) == 1 {
                        let _ = job.response_tx.send(ResultEvent::Done {
                            request_id: job.request_id.clone(),
                            total: job.total,
                        });
                    }
                }

                eprintln!("worker {worker_id} 退出");
            });
        }

        Self { workers }
    }

    fn submit(&self, request: PluginRequest) -> UnboundedReceiver<ResultEvent> {
        let (response_tx, response_rx) = unbounded_channel::<ResultEvent>();
        let pending = Arc::new(AtomicUsize::new(request.items.len()));
        let total = request.items.len();

        for item in request.items {
            let idx = self.pick_worker(&item.plugin_name);
            let job = WorkerJob {
                request_id: request.request_id.clone(),
                item,
                pending: pending.clone(),
                total,
                response_tx: response_tx.clone(),
            };

            if let Err(err) = self.workers[idx].send(job) {
                let _ = response_tx.send(ResultEvent::Item {
                    request_id: request.request_id.clone(),
                    item_id: 0,
                    ok: false,
                    data: None,
                    error: Some(format!("投递任务失败: {err}")),
                });
            }
        }

        response_rx
    }

    fn pick_worker(&self, plugin_name: &str) -> usize {
        let mut hasher = DefaultHasher::new();
        plugin_name.hash(&mut hasher);
        (hasher.finish() as usize) % self.workers.len()
    }
}

fn invoke_one(host: &HostRuntime, item: &InvokeItem) -> Result<Value, String> {
    let name_json = serde_json::to_string(&item.plugin_name).map_err(|e| e.to_string())?;
    let function_json = serde_json::to_string(&item.function).map_err(|e| e.to_string())?;
    let args_json = serde_json::to_string(&item.args).map_err(|e| e.to_string())?;

    let script = format!(
        r#"
        (async () => {{
          try {{
            const data = await globalThis.__plugin_invoke({name_json}, {function_json}, {args_json});
            return JSON.stringify({{ ok: true, data }});
          }} catch (err) {{
            return JSON.stringify({{ ok: false, error: String(err && err.message ? err.message : err) }});
          }}
        }})()
        "#
    );

    let raw = host.eval_async(&script).map_err(|e| e.to_string())?;
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

fn plugin_bootstrap_script() -> &'static str {
    r#"
    (async () => {
      const handlers = {
        test1: {
          "1": async (arg) => ({ doubled: Number(arg) * 2 }),
          "2": async (arg) => ({ upper: String(arg).toUpperCase() }),
        },
        test2: {
          "1": async (arg) => ({ len: JSON.stringify(arg).length }),
        }
      };

      plugin.register({ name: "test1", version: "1.0.0", apiVersion: 1 });
      plugin.register({ name: "test2", version: "1.0.0", apiVersion: 1 });

      globalThis.__plugin_invoke = async (name, fnId, args) => {
        const pluginImpl = handlers[name];
        if (!pluginImpl) throw new Error(`插件不存在: ${name}`);
        const fn = pluginImpl[String(fnId)];
        if (typeof fn !== "function") {
          throw new Error(`插件 ${name} 不支持函数 ${fnId}`);
        }
        return fn(args);
      };

      return "ok";
    })()
    "#
}

async fn stream_request(label: &str, manager: Arc<PluginManager>, request: PluginRequest) {
    let mut rx = manager.submit(request);
    while let Some(event) = rx.recv().await {
        match event {
            ResultEvent::Item {
                request_id,
                item_id,
                ok,
                data,
                error,
            } => {
                println!(
                    "[{label}] request={request_id} item={item_id} ok={ok} data={:?} error={:?}",
                    data, error
                );
            }
            ResultEvent::Done { request_id, total } => {
                println!("[{label}] request={request_id} done total={total}");
                break;
            }
        }
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let manager = Arc::new(PluginManager::new(2));

    let req_a = PluginRequest {
        request_id: "req-a".to_string(),
        items: vec![
            InvokeItem {
                item_id: 1,
                plugin_name: "test1".to_string(),
                function: "1".to_string(),
                args: json!(123),
            },
            InvokeItem {
                item_id: 2,
                plugin_name: "test1".to_string(),
                function: "2".to_string(),
                args: json!("hello"),
            },
            InvokeItem {
                item_id: 3,
                plugin_name: "test2".to_string(),
                function: "1".to_string(),
                args: json!({ "k": "v", "n": 42 }),
            },
        ],
    };

    let req_b = PluginRequest {
        request_id: "req-b".to_string(),
        items: vec![
            InvokeItem {
                item_id: 11,
                plugin_name: "test1".to_string(),
                function: "1".to_string(),
                args: json!(999),
            },
            InvokeItem {
                item_id: 12,
                plugin_name: "test2".to_string(),
                function: "1".to_string(),
                args: json!([1, 2, 3, 4, 5]),
            },
        ],
    };

    let t1 = tokio::spawn(stream_request("A", manager.clone(), req_a));
    let t2 = tokio::spawn(stream_request("B", manager.clone(), req_b));

    let _ = tokio::join!(t1, t2);
}
