use rquickjs::{function::Func, Context, Function, Runtime};
use serde::de::DeserializeOwned;
use std::future::IntoFuture;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use tokio::sync::oneshot;

use crate::web_runtime::{
    fs_task_drop_evented, fs_task_start_evented, http_request_drop_evented,
    http_request_start_evented, install_host_bindings, wasi_run_drop_evented,
    wasi_run_start_evented, AXIOS_BUNDLE, WEB_POLYFILL,
};

const ASYNC_TASK_DISPATCHER_JS: &str = r#"(function () {
  globalThis.__host_runtime_dispatch_task = function (__runtimeId, __taskId, __source) {
    let __value;
    try {
      __value = (0, eval)(__source);
    } catch (err) {
      let __msg;
      try {
        __msg = String(err && (err.stack || err.message) ? (err.stack || err.message) : err);
      } catch (_err) {
        __msg = "task eval error";
      }
      globalThis.__host_runtime_task_complete(__runtimeId, __taskId, false, __msg);
      return;
    }

    Promise.resolve(__value).then(
      (result) => {
        let __out;
        if (typeof result === "string") {
          __out = result;
        } else if (result === undefined) {
          __out = "undefined";
        } else {
          try {
            __out = JSON.stringify(result);
            if (__out === undefined) __out = String(result);
          } catch (_err) {
            __out = String(result);
          }
        }
        globalThis.__host_runtime_task_complete(__runtimeId, __taskId, true, __out);
      },
      (err) => {
        let __msg;
        try {
          __msg = String(err && (err.stack || err.message) ? (err.stack || err.message) : err);
        } catch (_err) {
          __msg = "task rejected";
        }
        globalThis.__host_runtime_task_complete(__runtimeId, __taskId, false, __msg);
      }
    );
  };
})();"#;

pub struct HostRuntime {
    runtime: Runtime,
    context: Context,
}

#[derive(Debug, Clone)]
pub struct RuntimeTaskStats {
    pub pending: usize,
    pub running: usize,
    pub done: usize,
    pub dropped: usize,
}

pub struct AsyncHostRuntime {
    runtime_id: u64,
    tx: mpsc::Sender<WorkerSignal>,
    states: Arc<Mutex<HashMap<u64, TaskState>>>,
    waiters: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    next_id: AtomicU64,
}

pub struct RuntimeTaskHandle {
    id: u64,
    rx: Option<oneshot::Receiver<Result<String, String>>>,
    states: Arc<Mutex<HashMap<u64, TaskState>>>,
    waiters: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    tx: mpsc::Sender<WorkerSignal>,
    drop_cleanup: bool,
}

pub struct RuntimeJsonTaskHandle<T> {
    inner: RuntimeTaskHandle,
    _marker: PhantomData<T>,
}

enum AsyncCommand {
    Submit { id: u64, script: String },
    Drop { id: u64 },
    Shutdown,
}

enum HostEvent {
    HttpCompleted { id: u64, payload: String },
    FsCompleted { id: u64, payload: String },
    WasiCompleted { id: u64, payload: String },
}

enum WorkerSignal {
    Command(AsyncCommand),
    HostEvent(HostEvent),
}

#[derive(Debug, Clone)]
enum TaskState {
    Pending,
    Running,
    Done(Result<String, String>),
    Dropped,
}

static ASYNC_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);
static ASYNC_RUNTIME_SHARED: OnceLock<Mutex<HashMap<u64, Arc<RuntimeShared>>>> = OnceLock::new();

struct RuntimeShared {
    states: Arc<Mutex<HashMap<u64, TaskState>>>,
    waiters: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
}

impl HostRuntime {
    pub fn new(load_axios: bool) -> Result<Self, rquickjs::Error> {
        let runtime = Runtime::new()?;
        let context = Context::full(&runtime)?;

        context.with(|ctx| {
            install_host_bindings(&ctx)?;
            ctx.eval::<(), _>(WEB_POLYFILL)?;
            if load_axios && !AXIOS_BUNDLE.is_empty() {
                ctx.eval::<(), _>(AXIOS_BUNDLE)?;
            }
            Ok::<(), rquickjs::Error>(())
        })?;

        Ok(Self { runtime, context })
    }

    pub fn submit_async_task(
        &self,
        runtime_id: u64,
        task_id: u64,
        script: &str,
    ) -> Result<(), String> {
        self.context
            .with(|ctx| {
                let globals = ctx.globals();
                let dispatch: Function = globals.get("__host_runtime_dispatch_task")?;
                dispatch.call::<_, ()>((runtime_id, task_id, script))
            })
            .map_err(|e| format!("任务提交到 JS 失败: {e}"))
    }

    pub fn pump_jobs(&self, max_jobs: usize) -> Result<usize, String> {
        let mut executed = 0usize;
        while executed < max_jobs && self.runtime.is_job_pending() {
            match self.runtime.execute_pending_job() {
                Ok(true) => executed += 1,
                Ok(false) => break,
                Err(err) => return Err(format!("执行 JS event loop job 失败: {err}")),
            }
        }
        Ok(executed)
    }

    pub fn with_context<R>(
        &self,
        f: impl for<'js> FnOnce(rquickjs::Ctx<'js>) -> Result<R, rquickjs::Error>,
    ) -> Result<R, rquickjs::Error> {
        self.context.with(f)
    }
}

impl AsyncHostRuntime {
    pub fn new(load_axios: bool) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<WorkerSignal>();
        let tx_for_worker = tx.clone();
        let states = Arc::new(Mutex::new(HashMap::<u64, TaskState>::new()));
        let states_for_worker = Arc::clone(&states);
        let waiters = Arc::new(Mutex::new(HashMap::<
            u64,
            oneshot::Sender<Result<String, String>>,
        >::new()));
        let waiters_for_worker = Arc::clone(&waiters);
        let runtime_id = ASYNC_RUNTIME_ID.fetch_add(1, Ordering::SeqCst);
        register_runtime_shared(
            runtime_id,
            Arc::new(RuntimeShared {
                states: Arc::clone(&states),
                waiters: Arc::clone(&waiters),
            }),
        );
        let (init_tx, init_rx) = mpsc::channel::<Result<(), String>>();

        thread::spawn(move || {
            let host = match HostRuntime::new(load_axios) {
                Ok(host) => host,
                Err(err) => {
                    let _ = init_tx.send(Err(format!("初始化 HostRuntime 失败: {err}")));
                    return;
                }
            };

            if let Err(err) = install_async_runtime_bindings(&host, tx_for_worker.clone()) {
                let _ = init_tx.send(Err(err));
                return;
            }

            let _ = init_tx.send(Ok(()));

            let mut running = true;
            while running {
                loop {
                    match rx.try_recv() {
                        Ok(signal) => {
                            running = handle_worker_signal(
                                signal,
                                &host,
                                runtime_id,
                                &states_for_worker,
                                &waiters_for_worker,
                            );
                            if !running {
                                break;
                            }
                        }
                        Err(mpsc::TryRecvError::Empty) => break,
                        Err(mpsc::TryRecvError::Disconnected) => {
                            running = false;
                            break;
                        }
                    }
                }

                if !running {
                    break;
                }

                match host.pump_jobs(2048) {
                    Ok(jobs) if jobs > 0 => continue,
                    Ok(_) => {}
                    Err(err) => {
                        fail_all_active_tasks(&states_for_worker, &waiters_for_worker, err);
                        break;
                    }
                }

                if !running {
                    break;
                }

                match rx.recv() {
                    Ok(signal) => {
                        running = handle_worker_signal(
                            signal,
                            &host,
                            runtime_id,
                            &states_for_worker,
                            &waiters_for_worker,
                        );
                    }
                    Err(_) => break,
                }
            }
        });

        match init_rx.recv() {
            Ok(Ok(())) => Ok(Self {
                runtime_id,
                tx,
                states,
                waiters,
                next_id: AtomicU64::new(1),
            }),
            Ok(Err(err)) => {
                unregister_runtime_shared(runtime_id);
                Err(err)
            }
            Err(_) => {
                unregister_runtime_shared(runtime_id);
                Err("初始化 HostRuntime 失败: worker 提前退出".to_string())
            }
        }
    }

    pub fn spawn(&self, script: impl Into<String>) -> Result<RuntimeTaskHandle, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (result_tx, result_rx) = oneshot::channel::<Result<String, String>>();

        {
            let mut guard = self
                .states
                .lock()
                .map_err(|_| "提交任务失败: 状态锁已损坏".to_string())?;
            guard.insert(id, TaskState::Pending);
        }

        {
            let mut guard = self
                .waiters
                .lock()
                .map_err(|_| "提交任务失败: 等待器锁已损坏".to_string())?;
            guard.insert(id, result_tx);
        }

        if self
            .tx
            .send(WorkerSignal::Command(AsyncCommand::Submit {
                id,
                script: script.into(),
            }))
            .is_err()
        {
            if let Ok(mut guard) = self.states.lock() {
                guard.remove(&id);
            }
            if let Ok(mut guard) = self.waiters.lock() {
                guard.remove(&id);
            }
            return Err("提交任务失败: worker 不可用".to_string());
        }

        Ok(RuntimeTaskHandle {
            id,
            rx: Some(result_rx),
            states: Arc::clone(&self.states),
            waiters: Arc::clone(&self.waiters),
            tx: self.tx.clone(),
            drop_cleanup: true,
        })
    }

    pub fn spawn_json<T>(&self, script: impl Into<String>) -> Result<RuntimeJsonTaskHandle<T>, String>
    where
        T: DeserializeOwned + Send + 'static,
    {
        let inner = self.spawn(script)?;
        Ok(RuntimeJsonTaskHandle {
            inner,
            _marker: PhantomData,
        })
    }

    pub fn cancel(&self, id: u64) -> bool {
        if self.tx.send(WorkerSignal::Command(AsyncCommand::Drop { id })).is_err() {
            return false;
        }
        true
    }

    pub fn stats(&self) -> RuntimeTaskStats {
        let Ok(guard) = self.states.lock() else {
            return RuntimeTaskStats {
                pending: 0,
                running: 0,
                done: 0,
                dropped: 0,
            };
        };

        let mut pending = 0usize;
        let mut running = 0usize;
        let mut done = 0usize;
        let mut dropped = 0usize;

        for state in guard.values() {
            match state {
                TaskState::Pending => pending += 1,
                TaskState::Running => running += 1,
                TaskState::Done(_) => done += 1,
                TaskState::Dropped => dropped += 1,
            }
        }

        RuntimeTaskStats {
            pending,
            running,
            done,
            dropped,
        }
    }
}

impl Drop for AsyncHostRuntime {
    fn drop(&mut self) {
        let _ = self.tx.send(WorkerSignal::Command(AsyncCommand::Shutdown));
        unregister_runtime_shared(self.runtime_id);
    }
}

impl RuntimeTaskHandle {
    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn wait(mut self) -> Result<String, String> {
        let Some(rx) = self.rx.take() else {
            self.drop_cleanup = false;
            clear_task_state(&self.states, self.id);
            return Err("等待任务结果失败: 任务句柄已失效".to_string());
        };

        let out = match rx.blocking_recv() {
            Ok(result) => {
                clear_task_state(&self.states, self.id);
                result
            }
            Err(_) => {
                clear_task_state(&self.states, self.id);
                Err("等待任务结果失败: runtime 已关闭".to_string())
            }
        };

        self.drop_cleanup = false;
        out
    }

    pub async fn wait_async(mut self) -> Result<String, String> {
        let Some(rx) = self.rx.take() else {
            self.drop_cleanup = false;
            clear_task_state(&self.states, self.id);
            return Err("等待任务结果失败: 任务句柄已失效".to_string());
        };

        let out = match rx.await {
            Ok(result) => {
                clear_task_state(&self.states, self.id);
                result
            }
            Err(_) => {
                clear_task_state(&self.states, self.id);
                Err("等待任务结果失败: runtime 已关闭".to_string())
            }
        };

        self.drop_cleanup = false;
        out
    }
}

impl Drop for RuntimeTaskHandle {
    fn drop(&mut self) {
        if !self.drop_cleanup {
            return;
        }

        remove_waiter(&self.waiters, self.id);

        if is_task_active(&self.states, self.id) {
            if self
                .tx
                .send(WorkerSignal::Command(AsyncCommand::Drop { id: self.id }))
                .is_err()
            {
                clear_task_state(&self.states, self.id);
            }
        } else {
            clear_task_state(&self.states, self.id);
        }
    }
}

impl IntoFuture for RuntimeTaskHandle {
    type Output = Result<String, String>;
    type IntoFuture =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.wait_async().await })
    }
}

impl<T> RuntimeJsonTaskHandle<T>
where
    T: DeserializeOwned + Send + 'static,
{
    pub fn id(&self) -> u64 {
        self.inner.id()
    }

    pub fn wait(self) -> Result<T, String> {
        parse_json_payload(self.inner.wait())
    }

    pub async fn wait_async(self) -> Result<T, String> {
        parse_json_payload(self.inner.wait_async().await)
    }
}

impl<T> IntoFuture for RuntimeJsonTaskHandle<T>
where
    T: DeserializeOwned + Send + 'static,
{
    type Output = Result<T, String>;
    type IntoFuture = std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, String>> + Send>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(async move { self.wait_async().await })
    }
}

fn parse_json_payload<T>(raw: Result<String, String>) -> Result<T, String>
where
    T: DeserializeOwned,
{
    match raw {
        Ok(payload) => serde_json::from_str(&payload)
            .map_err(|e| format!("解析 JSON 任务结果失败: {e}; payload={payload}")),
        Err(err) => Err(err),
    }
}

fn async_runtime_shared() -> &'static Mutex<HashMap<u64, Arc<RuntimeShared>>> {
    ASYNC_RUNTIME_SHARED.get_or_init(|| Mutex::new(HashMap::new()))
}

fn register_runtime_shared(runtime_id: u64, shared: Arc<RuntimeShared>) {
    if let Ok(mut guard) = async_runtime_shared().lock() {
        guard.insert(runtime_id, shared);
    }
}

fn unregister_runtime_shared(runtime_id: u64) {
    if let Ok(mut guard) = async_runtime_shared().lock() {
        guard.remove(&runtime_id);
    }
}

fn with_runtime_shared<F>(runtime_id: u64, f: F)
where
    F: FnOnce(&Arc<RuntimeShared>),
{
    let Some(shared) = async_runtime_shared()
        .lock()
        .ok()
        .and_then(|guard| guard.get(&runtime_id).cloned())
    else {
        return;
    };

    f(&shared);
}

fn async_runtime_task_complete(runtime_id: u64, task_id: u64, ok: bool, payload: String) {
    let outcome = if ok { Ok(payload) } else { Err(payload) };
    with_runtime_shared(runtime_id, |shared| {
        finalize_task_and_notify(shared, task_id, outcome)
    });
}

fn install_async_runtime_bindings(
    host: &HostRuntime,
    signal_tx: mpsc::Sender<WorkerSignal>,
) -> Result<(), String> {
    host.with_context(|ctx| {
        let globals = ctx.globals();
        globals.set(
            "__host_runtime_task_complete",
            Func::from(async_runtime_task_complete),
        )?;
        install_evented_host_bindings_worker(&ctx, signal_tx.clone())?;
        ctx.eval::<(), _>(ASYNC_TASK_DISPATCHER_JS)?;
        Ok::<(), rquickjs::Error>(())
    })
    .map_err(|e| format!("安装 AsyncHostRuntime 绑定失败: {e}"))
}

fn install_evented_host_bindings_worker(
    ctx: &rquickjs::Ctx<'_>,
    signal_tx: mpsc::Sender<WorkerSignal>,
) -> Result<(), rquickjs::Error> {
    let globals = ctx.globals();

    let http_tx = signal_tx.clone();
    globals.set(
        "__http_request_start_evented",
        Function::new(
            ctx.clone(),
            move |method: String, url: String, headers_json: String, body: Option<String>| {
                let tx = http_tx.clone();
                http_request_start_evented(method, url, headers_json, body, move |id, payload| {
                    let _ = tx.send(WorkerSignal::HostEvent(HostEvent::HttpCompleted {
                        id,
                        payload,
                    }));
                })
            },
        )?,
    )?;
    globals.set("__http_request_drop_evented", Func::from(http_request_drop_evented))?;

    let fs_tx = signal_tx.clone();
    globals.set(
        "__fs_task_start_evented",
        Function::new(ctx.clone(), move |op: String, args_json: String| {
            let tx = fs_tx.clone();
            fs_task_start_evented(op, args_json, move |id, payload| {
                let _ = tx.send(WorkerSignal::HostEvent(HostEvent::FsCompleted { id, payload }));
            })
        })?,
    )?;
    globals.set("__fs_task_drop_evented", Func::from(fs_task_drop_evented))?;

    let wasi_tx = signal_tx;
    globals.set(
        "__wasi_run_start_evented",
        Function::new(
            ctx.clone(),
            move |module_id: u64,
                  stdin_id: Option<u64>,
                  args_json: Option<String>,
                  consume_module: bool| {
                let tx = wasi_tx.clone();
                wasi_run_start_evented(
                    module_id,
                    stdin_id,
                    args_json,
                    consume_module,
                    move |id, payload| {
                        let _ = tx.send(WorkerSignal::HostEvent(HostEvent::WasiCompleted {
                            id,
                            payload,
                        }));
                    },
                )
            },
        )?,
    )?;
    globals.set("__wasi_run_drop_evented", Func::from(wasi_run_drop_evented))?;

    Ok(())
}

fn handle_worker_signal(
    signal: WorkerSignal,
    host: &HostRuntime,
    runtime_id: u64,
    states: &Arc<Mutex<HashMap<u64, TaskState>>>,
    waiters: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
) -> bool {
    match signal {
        WorkerSignal::Command(cmd) => {
            handle_worker_command(cmd, host, runtime_id, states, waiters)
        }
        WorkerSignal::HostEvent(event) => {
            handle_host_event(host, event);
            true
        }
    }
}

fn handle_worker_command(
    cmd: AsyncCommand,
    host: &HostRuntime,
    runtime_id: u64,
    states: &Arc<Mutex<HashMap<u64, TaskState>>>,
    waiters: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
) -> bool {
    match cmd {
        AsyncCommand::Submit { id, script } => {
            if !mark_running_if_available(states, id) {
                return true;
            }

            if let Err(err) = host.submit_async_task(runtime_id, id, &script) {
                finalize_task_with_waiter(states, waiters, id, Err(err));
            }
            true
        }
        AsyncCommand::Drop { id } => {
            mark_dropped_and_notify(states, waiters, id);
            true
        }
        AsyncCommand::Shutdown => false,
    }
}

fn handle_host_event(host: &HostRuntime, event: HostEvent) {
    let result = host.with_context(|ctx| handle_host_event_in_ctx(ctx, event));

    let _ = result;
}

fn handle_host_event_in_ctx(ctx: rquickjs::Ctx<'_>, event: HostEvent) -> Result<(), rquickjs::Error> {
    let globals = ctx.globals();
    match event {
        HostEvent::HttpCompleted { id, payload } => {
            let func: Function = globals.get("__host_runtime_http_complete")?;
            func.call::<_, ()>((id, payload))
        }
        HostEvent::FsCompleted { id, payload } => {
            let func: Function = globals.get("__host_runtime_fs_complete")?;
            func.call::<_, ()>((id, payload))
        }
        HostEvent::WasiCompleted { id, payload } => {
            let func: Function = globals.get("__host_runtime_wasi_complete")?;
            func.call::<_, ()>((id, payload))
        }
    }
}

fn fail_all_active_tasks(
    states: &Arc<Mutex<HashMap<u64, TaskState>>>,
    waiters: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    message: String,
) {
    let Ok(mut guard) = states.lock() else {
        return;
    };

    let mut failed_ids = Vec::new();
    for (id, state) in guard.iter_mut() {
        match state {
            TaskState::Pending | TaskState::Running => {
                *state = TaskState::Done(Err(message.clone()));
                failed_ids.push(*id);
            }
            TaskState::Done(_) | TaskState::Dropped => {}
        }
    }

    drop(guard);
    for id in failed_ids {
        let notified = notify_waiter(waiters, id, Err(message.clone()));
        if !notified {
            clear_task_state(states, id);
        }
    }
}

fn mark_dropped_and_notify(
    states: &Arc<Mutex<HashMap<u64, TaskState>>>,
    waiters: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    id: u64,
) {
    let Ok(mut guard) = states.lock() else {
        return;
    };

    let Some(state) = guard.get(&id) else {
        return;
    };

    if !matches!(state, TaskState::Pending | TaskState::Running) {
        return;
    }

    guard.insert(id, TaskState::Dropped);
    drop(guard);
    let notified = notify_waiter(waiters, id, Err("task dropped".to_string()));
    if !notified {
        clear_task_state(states, id);
    }
}

fn finalize_task_with_waiter(
    states: &Arc<Mutex<HashMap<u64, TaskState>>>,
    waiters: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    id: u64,
    outcome: Result<String, String>,
) {
    if finalize_task(states, id, outcome) {
        let notified = notify_waiter(waiters, id, read_done_outcome(states, id));
        if !notified {
            clear_task_state(states, id);
        }
    }
}

fn finalize_task_and_notify(shared: &RuntimeShared, id: u64, outcome: Result<String, String>) {
    finalize_task_with_waiter(&shared.states, &shared.waiters, id, outcome);
}

fn notify_waiter(
    waiters: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    id: u64,
    outcome: Result<String, String>,
) -> bool {
    let sender = waiters.lock().ok().and_then(|mut guard| guard.remove(&id));
    if let Some(tx) = sender {
        let _ = tx.send(outcome);
        true
    } else {
        false
    }
}

fn read_done_outcome(
    states: &Arc<Mutex<HashMap<u64, TaskState>>>,
    id: u64,
) -> Result<String, String> {
    let Ok(guard) = states.lock() else {
        return Err("读取任务结果失败: 状态锁已损坏".to_string());
    };

    match guard.get(&id) {
        Some(TaskState::Done(Ok(value))) => Ok(value.clone()),
        Some(TaskState::Done(Err(err))) => Err(err.clone()),
        Some(TaskState::Dropped) => Err("task dropped".to_string()),
        Some(TaskState::Pending) | Some(TaskState::Running) => {
            Err("读取任务结果失败: 任务尚未完成".to_string())
        }
        None => Err("读取任务结果失败: 任务不存在".to_string()),
    }
}

fn clear_task_state(states: &Arc<Mutex<HashMap<u64, TaskState>>>, id: u64) {
    if let Ok(mut guard) = states.lock() {
        guard.remove(&id);
    }
}

fn remove_waiter(
    waiters: &Arc<Mutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    id: u64,
) {
    if let Ok(mut guard) = waiters.lock() {
        guard.remove(&id);
    }
}

fn is_task_active(states: &Arc<Mutex<HashMap<u64, TaskState>>>, id: u64) -> bool {
    let Ok(guard) = states.lock() else {
        return false;
    };

    matches!(guard.get(&id), Some(TaskState::Pending | TaskState::Running))
}

fn mark_running_if_available(states: &Arc<Mutex<HashMap<u64, TaskState>>>, id: u64) -> bool {
    let Ok(mut guard) = states.lock() else {
        return false;
    };

    match guard.get(&id) {
        Some(TaskState::Dropped) | None => false,
        _ => {
            guard.insert(id, TaskState::Running);
            true
        }
    }
}

fn finalize_task(
    states: &Arc<Mutex<HashMap<u64, TaskState>>>,
    id: u64,
    outcome: Result<String, String>,
) -> bool {
    let Ok(mut guard) = states.lock() else {
        return false;
    };

    match guard.get(&id) {
        Some(TaskState::Dropped) | None => {
            let _ = guard.remove(&id);
            false
        }
        _ => {
            guard.insert(id, TaskState::Done(outcome));
            true
        }
    }
}
