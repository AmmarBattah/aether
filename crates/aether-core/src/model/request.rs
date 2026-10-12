use std::{collections::BTreeMap, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Message, ModelRef};

/// A system prompt segment. Segments are ordered; a caching adapter may
/// place a cache breakpoint after any segment flagged `cache`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SystemBlock {
    pub text: String,
    #[serde(default)]
    pub cache: bool,
}

/// Tool definition as exposed to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema for the tool input.
    pub input_schema: Value,
    /// Ask the provider to guarantee schema-valid arguments when it can.
    #[serde(default)]
    pub strict: bool,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    #[default]
    Auto,
    None,
}

/// Reasoning depth. Providers map this to their own controls.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Effort {
    Low,
    Medium,
    High,
    #[serde(alias = "xhigh")]
    XHigh,
    Max,
}

impl Effort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::XHigh => "xhigh",
            Self::Max => "max",
        }
    }
}

impl fmt::Display for Effort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Effort {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" | "med" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" | "x-high" | "extra_high" => Ok(Self::XHigh),
            "max" => Ok(Self::Max),
            other => Err(format!("unknown effort level `{other}`")),
        }
    }
}

/// Claude Code's prompt-cache write tier for a headless (`-p`/stream-json)
/// engine turn. The CLI silently defaults subscription logins to `OneHour`
/// (2x write price) unless told otherwise; aether's turn gaps are almost
/// always under 5 minutes, where that tier's price does not pay off.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
pub enum PromptCacheTtl {
    #[default]
    #[serde(rename = "5m")]
    FiveMinutes,
    #[serde(rename = "1h")]
    OneHour,
}

impl PromptCacheTtl {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FiveMinutes => "5m",
            Self::OneHour => "1h",
        }
    }
}

/// How the model should think. Adapters honor what the capability profile
/// supports and report the rest as unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ThinkingMode {
    Off,
    Adaptive,
    Budget { tokens: u32 },
}

/// One request to a model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModelRequest {
    pub model: ModelRef,
    #[serde(default)]
    pub system: Vec<SystemBlock>,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    #[serde(default)]
    pub tool_choice: ToolChoice,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    /// Opaque request metadata (user id for abuse tracking, session id).
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    /// Provider-specific parameters keyed `provider.name`, e.g. `anthropic.betas`.
    #[serde(default)]
    pub extensions: BTreeMap<String, Value>,
}

impl ModelRequest {
    pub fn new(model: ModelRef, messages: Vec<Message>) -> Self {
        Self {
            model,
            system: Vec::new(),
            messages,
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            max_output_tokens: None,
            effort: None,
            thinking: None,
            temperature: None,
            stop_sequences: Vec::new(),
            metadata: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }
}
