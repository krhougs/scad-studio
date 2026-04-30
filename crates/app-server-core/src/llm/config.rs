use serde::Deserialize;
use std::{env, fmt, path::PathBuf};
use tokio::fs;

#[derive(Clone)]
pub struct RigAgentConfig {
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
    pub max_tokens: u64,
    pub temperature: f64,
    pub reasoning_effort: Option<String>,
}

impl std::fmt::Debug for RigAgentConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RigAgentConfig")
            .field("api_key", &"***")
            .field("model", &self.model)
            .field("timeout_secs", &self.timeout_secs)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .field("reasoning_effort", &self.reasoning_effort)
            .finish()
    }
}

#[derive(Debug)]
pub struct RigAgentConfigError {
    pub message: String,
}

impl fmt::Display for RigAgentConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Deserialize)]
struct RigAgentConfigFile {
    api_key: String,
    model: Option<String>,
    timeout_secs: Option<u64>,
    max_tokens: Option<u64>,
    temperature: Option<f64>,
    reasoning_effort: Option<String>,
}

impl RigAgentConfigFile {
    fn into_config(self) -> Result<RigAgentConfig, RigAgentConfigError> {
        if self.api_key.is_empty() {
            return Err(RigAgentConfigError {
                message: "api_key is empty".into(),
            });
        }
        Ok(RigAgentConfig {
            api_key: self.api_key,
            model: self.model.unwrap_or_else(default_model),
            timeout_secs: self.timeout_secs.unwrap_or(120),
            max_tokens: self.max_tokens.unwrap_or(8192),
            temperature: self.temperature.unwrap_or(0.7),
            reasoning_effort: non_empty(self.reasoning_effort),
        })
    }
}

async fn load_from_file(path: PathBuf) -> Result<RigAgentConfig, RigAgentConfigError> {
    let content = fs::read_to_string(path.clone())
        .await
        .map_err(|e| RigAgentConfigError {
            message: format!(
                "Cannot read Rig OpenAI Responses config file {}: {e}",
                path.display()
            ),
        })?;
    let file: RigAgentConfigFile = toml::from_str(&content).map_err(|e| RigAgentConfigError {
        message: format!(
            "Cannot parse Rig OpenAI Responses config file {}: {e}",
            path.display()
        ),
    })?;
    file.into_config()
}

fn load_from_env() -> Option<RigAgentConfig> {
    let api_key = env::var("BUDN_AGENT_OPENAI_API_KEY")
        .or_else(|_| env::var("OPENAI_API_KEY"))
        .ok()?;
    if api_key.is_empty() {
        return None;
    }
    let model = env::var("BUDN_AGENT_MODEL").unwrap_or_else(|_| default_model());
    let timeout_secs = env::var("BUDN_AGENT_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let max_tokens = env::var("BUDN_AGENT_MAX_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8192);
    let temperature = env::var("BUDN_AGENT_TEMPERATURE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.7);
    let reasoning_effort = non_empty(env::var("BUDN_AGENT_REASONING_EFFORT").ok());

    Some(RigAgentConfig {
        api_key,
        model,
        timeout_secs,
        max_tokens,
        temperature,
        reasoning_effort,
    })
}

pub async fn load_rig_agent_config() -> Result<Option<RigAgentConfig>, RigAgentConfigError> {
    if let Ok(path) = env::var("BUDN_AGENT_CONFIG") {
        return load_from_file(PathBuf::from(path)).await.map(Some);
    }
    Ok(load_from_env())
}

fn default_model() -> String {
    "gpt-5.2".into()
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.and_then(|text| {
        let trimmed = text.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}
