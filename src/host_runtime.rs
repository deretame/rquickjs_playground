use rquickjs::{Context, Promise, Runtime};

use crate::web_runtime::{install_host_bindings, AXIOS_BUNDLE, WEB_POLYFILL};

pub struct HostRuntime {
    _runtime: Runtime,
    context: Context,
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

        Ok(Self {
            _runtime: runtime,
            context,
        })
    }

    pub fn eval_async(&self, script: &str) -> Result<String, rquickjs::Error> {
        self.context.with(|ctx| {
            let promise: Promise = ctx.eval(script)?;
            promise.finish()
        })
    }

    pub fn with_context<R>(
        &self,
        f: impl for<'js> FnOnce(rquickjs::Ctx<'js>) -> Result<R, rquickjs::Error>,
    ) -> Result<R, rquickjs::Error> {
        self.context.with(f)
    }
}
