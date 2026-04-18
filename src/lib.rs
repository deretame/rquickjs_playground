pub mod host_runtime;
pub mod web_runtime;

pub use host_runtime::{
    AsyncHostRuntime, RuntimeJsonTaskHandle, RuntimeTaskHandle, RuntimeTaskStats,
    configure_js_error_stack, js_error_stack_enabled,
};
pub use web_runtime::{
    HttpClientConfig, WebRuntimeOptions, configure_http_client, configure_log_http_endpoint,
    current_http_client_config, current_log_http_endpoint, polyfill_script,
    register_bridge_route_async_handler, unregister_bridge_route_handler,
};

#[cfg(test)]
mod tests;
