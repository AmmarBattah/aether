//! OAuth building blocks and the provider flows built from them.

pub mod callback;
pub mod codex;
pub mod device_code;
pub mod generic;
pub mod openrouter;
pub mod pkce;

pub use generic::{DeviceEndpoints, OAuthFlowConfig, OAuthTokens, TokenContentType};
