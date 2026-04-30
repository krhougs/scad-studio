use serde::Deserialize;
use std::{env, fmt, path::PathBuf};
use tokio::fs;

#[derive(Clone)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl std::fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmConfig")
            .field("base_url", &self.base_url)
            .field("api_key", &"***")
            .field("model", &self.model)
            .field("timeout_secs", &self.timeout_secs)
            .field("max_tokens", &self.max_tokens)
            .field("temperature", &self.temperature)
            .finish()
    }
}

#[derive(Debug)]
pub struct LlmConfigError {
    pub message: String,
}

impl fmt::Display for LlmConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

#[derive(Deserialize)]
struct LlmConfigFile {
    base_url: String,
    api_key: String,
    model: Option<String>,
    timeout_secs: Option<u64>,
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    variant: Option<String>,
    service_level: Option<String>,
}

impl LlmConfigFile {
    fn into_config(self) -> Result<LlmConfig, LlmConfigError> {
        if self.base_url.is_empty() {
            return Err(LlmConfigError {
                message: "base_url is empty".into(),
            });
        }
        if self.api_key.is_empty() {
            return Err(LlmConfigError {
                message: "api_key is empty".into(),
            });
        }
        let base_model = self.model.unwrap_or_else(|| "gpt-4o".into());
        let model = build_model_string(
            &base_model,
            self.variant.as_deref(),
            self.service_level.as_deref(),
        );
        Ok(LlmConfig {
            base_url: self.base_url,
            api_key: self.api_key,
            model,
            timeout_secs: self.timeout_secs.unwrap_or(120),
            max_tokens: self.max_tokens.unwrap_or(4096),
            temperature: self.temperature.unwrap_or(0.7),
        })
    }
}

pub fn build_model_string(
    base: &str,
    variant: Option<&str>,
    service_level: Option<&str>,
) -> String {
    let mut model = base.to_string();
    if let Some(v) = variant {
        if !v.is_empty() {
            model = format!("{model}:{v}");
        }
    }
    if let Some(sl) = service_level {
        if !sl.is_empty() {
            model = format!("{model}:{sl}");
        }
    }
    model
}

async fn load_from_file(path: PathBuf) -> Result<LlmConfig, LlmConfigError> {
    let content = fs::read_to_string(path.clone())
        .await
        .map_err(|e| LlmConfigError {
            message: format!("Cannot read LLM config file {}: {e}", path.display()),
        })?;
    let file: LlmConfigFile = toml::from_str(&content).map_err(|e| LlmConfigError {
        message: format!("Cannot parse LLM config file {}: {e}", path.display()),
    })?;
    file.into_config()
}

fn load_from_env() -> Option<LlmConfig> {
    let base_url = env::var("BUDN_LLM_BASE_URL").ok()?;
    let api_key = env::var("BUDN_LLM_API_KEY").ok()?;
    if base_url.is_empty() || api_key.is_empty() {
        return None;
    }
    let base_model = env::var("BUDN_LLM_MODEL").unwrap_or_else(|_| "gpt-4o".into());
    let variant = env::var("BUDN_LLM_VARIANT").ok();
    let service_level = env::var("BUDN_LLM_SERVICE_LEVEL").ok();
    let model = build_model_string(&base_model, variant.as_deref(), service_level.as_deref());
    let timeout_secs = env::var("BUDN_LLM_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(120);
    let max_tokens = env::var("BUDN_LLM_MAX_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4096);
    let temperature: f32 = env::var("BUDN_LLM_TEMPERATURE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.7);

    Some(LlmConfig {
        base_url,
        api_key,
        model,
        timeout_secs,
        max_tokens,
        temperature,
    })
}

pub async fn load_llm_config() -> Result<Option<LlmConfig>, LlmConfigError> {
    if let Ok(path) = env::var("BUDN_LLM_CONFIG") {
        return load_from_file(PathBuf::from(path)).await.map(Some);
    }
    Ok(load_from_env())
}
