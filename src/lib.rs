pub mod host_runtime;
pub mod web_runtime;

pub use host_runtime::{
    AsyncHostRuntime, RuntimeJsonTaskHandle, RuntimeTaskHandle, RuntimeTaskStats,
};
pub use web_runtime::{
    HttpClientConfig, configure_http_client, current_http_client_config,
    register_flush_persistent_store_handler, register_load_persistent_store_handler,
};

#[cfg(test)]
mod tests;
