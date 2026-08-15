//! Token usage tracking service
//!
//! Tracks and persists token consumption statistics per model, session, and turn.

mod service;
mod subscriber;

pub use bitfun_services_core::token_usage::types;
pub use bitfun_services_core::token_usage::{
    ModelTokenStats, SessionTokenStats, TimeRange, TokenUsageQuery, TokenUsageRecord,
    TokenUsageSummary,
};
pub use service::TokenUsageService;
pub use subscriber::TokenUsageSubscriber;

use std::sync::{Arc, OnceLock};

static GLOBAL_TOKEN_USAGE_SERVICE: OnceLock<Arc<TokenUsageService>> = OnceLock::new();

/// Install the process-wide token usage service (called once at boot).
pub fn set_global_token_usage_service(service: Arc<TokenUsageService>) {
    let _ = GLOBAL_TOKEN_USAGE_SERVICE.set(service);
}

/// Get the process-wide token usage service, if installed.
pub fn get_global_token_usage_service() -> Option<Arc<TokenUsageService>> {
    GLOBAL_TOKEN_USAGE_SERVICE.get().cloned()
}
