use rig::prelude::ModelListingClient;
use serde::Deserialize;
use std::{collections::HashSet, env, fmt, path::PathBuf, time::Duration};
use tokio::{fs, time};

#[derive(Clone)]
pub struct RigAgentConfig {
    pub provider_id: String,
    pub provider_kind: AgentProviderKind,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u64,
    pub max_tokens: u64,
    pub temperature: f64,
    pub reasoning_effort: Option<String>,
    pub service_label: Option<String>,
    pub native_web_search: bool,
    pub anthropic_version: Option<String>,
    pub base_url: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct RigAgentConfigSelection {
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub reasoning_effort: Option<String>,
    pub service_label: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum AgentProviderKind {
    #[serde(rename = "openai_responses")]
    OpenAiResponses,
    #[serde(rename = "openai_completions")]
    OpenAiCompletions,
    #[serde(rename = "anthropic")]
    Anthropic,
}

impl AgentProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiResponses => "openai_responses",
            Self::OpenAiCompletions => "openai_completions",
            Self::Anthropic => "anthropic",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AgentModelSource {
    Manual,
    Discovered,
    DiscoveredWithOverride,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelDiscoveryStatus {
    Disabled,
    NotStarted,
    Succeeded,
    Failed(String),
}

#[derive(Clone, Debug)]
pub struct AgentConfigDefaults {
    pub timeout_secs: u64,
    pub max_tokens: u64,
    pub temperature: f64,
    pub native_web_search: bool,
    pub discover_models: bool,
}

#[derive(Clone, Debug)]
pub struct AgentProviderRegistry {
    pub active_provider_id: String,
    pub active_model_id: String,
    pub defaults: AgentConfigDefaults,
    pub providers: Vec<ResolvedAgentProvider>,
}

impl AgentProviderRegistry {
    pub fn provider(&self, id: &str) -> Option<&ResolvedAgentProvider> {
        self.providers.iter().find(|provider| provider.id == id)
    }

    pub fn active_provider(&self) -> Option<&ResolvedAgentProvider> {
        self.provider(&self.active_provider_id)
    }

    pub fn active_model(&self) -> Option<&ResolvedAgentModel> {
        self.active_provider()?
            .models
            .iter()
            .find(|model| model.id == self.active_model_id)
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedAgentProvider {
    pub id: String,
    pub kind: AgentProviderKind,
    pub api_key: Option<String>,
    pub api_key_env: String,
    pub anthropic_version: Option<String>,
    pub base_url: Option<String>,
    pub discover_models: bool,
    pub model_discovery_status: ModelDiscoveryStatus,
    defaults: AgentConfigDefaults,
    pub models: Vec<ResolvedAgentModel>,
}

#[derive(Clone, Debug)]
pub struct ResolvedAgentModel {
    pub id: String,
    pub label: Option<String>,
    pub max_tokens: u64,
    pub temperature: f64,
    pub reasoning_effort: Option<String>,
    pub service_label: Option<String>,
    pub native_web_search: bool,
    pub web_search_supported: bool,
    pub web_search_unsupported_reason: Option<String>,
    pub source: AgentModelSource,
    explicit: ModelExplicitFields,
}

#[derive(Clone, Debug)]
pub struct DiscoveredProviderModel {
    pub id: String,
    pub label: String,
    pub web_search_supported: bool,
}

#[derive(Clone, Debug, Default)]
struct ModelExplicitFields {
    max_tokens: bool,
    temperature: bool,
    reasoning_effort: bool,
    service_label: bool,
    native_web_search: bool,
    web_search_supported: bool,
    web_search_unsupported_reason: bool,
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
            .field("native_web_search", &self.native_web_search)
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
struct LegacyRigAgentConfigFile {
    api_key: String,
    model: Option<String>,
    timeout_secs: Option<u64>,
    max_tokens: Option<u64>,
    temperature: Option<f64>,
    reasoning_effort: Option<String>,
    native_web_search: Option<bool>,
}

impl LegacyRigAgentConfigFile {
    fn into_config(self) -> Result<RigAgentConfig, RigAgentConfigError> {
        if self.api_key.is_empty() {
            return Err(RigAgentConfigError {
                message: "api_key is empty".into(),
            });
        }
        Ok(RigAgentConfig {
            provider_id: "openai".into(),
            provider_kind: AgentProviderKind::OpenAiResponses,
            api_key: self.api_key,
            model: self.model.unwrap_or_else(default_model),
            timeout_secs: self.timeout_secs.unwrap_or(120),
            max_tokens: self.max_tokens.unwrap_or(8192),
            temperature: self.temperature.unwrap_or(0.7),
            reasoning_effort: non_empty(self.reasoning_effort),
            service_label: None,
            native_web_search: self.native_web_search.unwrap_or(true),
            anthropic_version: None,
            base_url: None,
        })
    }
}

#[derive(Deserialize)]
struct AgentsConfigFile {
    active_provider: String,
    active_model: String,
    defaults: Option<AgentConfigDefaultsFile>,
    providers: Vec<AgentProviderFile>,
}

#[derive(Default, Deserialize)]
struct AgentConfigDefaultsFile {
    timeout_secs: Option<u64>,
    max_tokens: Option<u64>,
    temperature: Option<f64>,
    native_web_search: Option<bool>,
    discover_models: Option<bool>,
}

#[derive(Deserialize)]
struct AgentProviderFile {
    id: String,
    kind: AgentProviderKind,
    api_key_env: String,
    base_url: Option<String>,
    anthropic_version: Option<String>,
    discover_models: Option<bool>,
    #[serde(default)]
    models: Vec<AgentModelFile>,
}

#[derive(Clone, Deserialize)]
struct AgentModelFile {
    id: String,
    label: Option<String>,
    max_tokens: Option<u64>,
    temperature: Option<f64>,
    reasoning_effort: Option<String>,
    service_label: Option<String>,
    native_web_search: Option<bool>,
    web_search_supported: Option<bool>,
    web_search_unsupported_reason: Option<String>,
}

impl AgentsConfigFile {
    fn into_registry(self) -> Result<AgentProviderRegistry, RigAgentConfigError> {
        let defaults = resolve_defaults(self.defaults);
        let providers = resolve_providers(self.providers, &defaults)?;
        let registry = AgentProviderRegistry {
            active_provider_id: require_non_empty("active_provider", self.active_provider)?,
            active_model_id: require_non_empty("active_model", self.active_model)?,
            defaults,
            providers,
        };
        validate_active_model(&registry)?;
        validate_active_provider_key(&registry)?;
        Ok(registry)
    }
}

fn resolve_defaults(defaults: Option<AgentConfigDefaultsFile>) -> AgentConfigDefaults {
    let defaults = defaults.unwrap_or_default();
    AgentConfigDefaults {
        timeout_secs: defaults.timeout_secs.unwrap_or(120),
        max_tokens: defaults.max_tokens.unwrap_or(8192),
        temperature: defaults.temperature.unwrap_or(0.7),
        native_web_search: defaults.native_web_search.unwrap_or(true),
        discover_models: defaults.discover_models.unwrap_or(true),
    }
}

fn resolve_providers(
    providers: Vec<AgentProviderFile>,
    defaults: &AgentConfigDefaults,
) -> Result<Vec<ResolvedAgentProvider>, RigAgentConfigError> {
    let mut seen = HashSet::new();
    providers
        .into_iter()
        .map(|provider| resolve_provider(provider, defaults, &mut seen))
        .collect()
}

fn resolve_provider(
    provider: AgentProviderFile,
    defaults: &AgentConfigDefaults,
    seen: &mut HashSet<String>,
) -> Result<ResolvedAgentProvider, RigAgentConfigError> {
    let id = require_non_empty("provider id", provider.id)?;
    if !seen.insert(id.clone()) {
        return config_error(format!("duplicate provider id `{id}`"));
    }
    validate_anthropic_version(&provider.kind, provider.anthropic_version.as_deref())?;
    let api_key_env = require_non_empty("api_key_env", provider.api_key_env)?;
    let api_key = read_api_key_env(&api_key_env);
    let models = resolve_models(provider.models, &provider.kind, defaults)?;
    let base_url = resolve_provider_base_url(&provider.kind, provider.base_url)?;
    Ok(ResolvedAgentProvider {
        id,
        kind: provider.kind.clone(),
        api_key,
        api_key_env,
        anthropic_version: resolve_anthropic_version(&provider.kind, provider.anthropic_version),
        base_url,
        discover_models: provider.discover_models.unwrap_or(defaults.discover_models),
        model_discovery_status: if provider.discover_models.unwrap_or(defaults.discover_models) {
            ModelDiscoveryStatus::NotStarted
        } else {
            ModelDiscoveryStatus::Disabled
        },
        defaults: defaults.clone(),
        models,
    })
}

fn resolve_models(
    models: Vec<AgentModelFile>,
    kind: &AgentProviderKind,
    defaults: &AgentConfigDefaults,
) -> Result<Vec<ResolvedAgentModel>, RigAgentConfigError> {
    let mut seen = HashSet::new();
    models
        .into_iter()
        .map(|model| resolve_model(model, kind, defaults, &mut seen))
        .collect()
}

fn resolve_model(
    model: AgentModelFile,
    kind: &AgentProviderKind,
    defaults: &AgentConfigDefaults,
    seen: &mut HashSet<String>,
) -> Result<ResolvedAgentModel, RigAgentConfigError> {
    let id = require_non_empty("model id", model.id)?;
    if !seen.insert(id.clone()) {
        return config_error(format!("duplicate model id `{id}`"));
    }
    let web_search_supported = provider_default_web_search_supported(kind)
        && model
            .web_search_supported
            .unwrap_or_else(|| provider_default_web_search_supported(kind));
    let explicit = ModelExplicitFields {
        max_tokens: model.max_tokens.is_some(),
        temperature: model.temperature.is_some(),
        reasoning_effort: model.reasoning_effort.is_some(),
        service_label: model.service_label.is_some(),
        native_web_search: model.native_web_search.is_some(),
        web_search_supported: model.web_search_supported.is_some(),
        web_search_unsupported_reason: model.web_search_unsupported_reason.is_some(),
    };
    let reason = web_search_reason(
        &id,
        web_search_supported,
        model.web_search_unsupported_reason,
    )?;
    Ok(ResolvedAgentModel {
        id,
        label: non_empty(model.label),
        max_tokens: model.max_tokens.unwrap_or(defaults.max_tokens),
        temperature: model.temperature.unwrap_or(defaults.temperature),
        reasoning_effort: non_empty(model.reasoning_effort),
        service_label: non_empty(model.service_label),
        native_web_search: model
            .native_web_search
            .unwrap_or(defaults.native_web_search),
        web_search_supported,
        web_search_unsupported_reason: reason,
        source: AgentModelSource::Manual,
        explicit,
    })
}

pub fn merge_provider_models(
    provider: &ResolvedAgentProvider,
    discovered: Vec<DiscoveredProviderModel>,
) -> Vec<ResolvedAgentModel> {
    let mut merged: Vec<_> = discovered
        .into_iter()
        .map(|model| discovered_to_resolved_model(provider, model))
        .collect();
    for manual in &provider.models {
        match merged.iter_mut().find(|model| model.id == manual.id) {
            Some(existing) => apply_manual_model_override(existing, manual),
            None => merged.push(manual.clone()),
        }
    }
    merged
}

pub fn apply_provider_model_discovery(
    provider: &ResolvedAgentProvider,
    discovered: Result<Vec<DiscoveredProviderModel>, String>,
) -> ResolvedAgentProvider {
    let mut provider = provider.clone();
    if !provider.discover_models {
        provider.model_discovery_status = ModelDiscoveryStatus::Disabled;
        return provider;
    }
    match discovered {
        Ok(models) => {
            provider.models = merge_provider_models(&provider, models);
            provider.model_discovery_status = ModelDiscoveryStatus::Succeeded;
        }
        Err(error) => {
            provider.model_discovery_status = ModelDiscoveryStatus::Failed(error);
        }
    }
    provider
}

fn discovered_to_resolved_model(
    provider: &ResolvedAgentProvider,
    discovered: DiscoveredProviderModel,
) -> ResolvedAgentModel {
    ResolvedAgentModel {
        id: discovered.id,
        label: Some(discovered.label),
        max_tokens: provider.defaults.max_tokens,
        temperature: provider.defaults.temperature,
        reasoning_effort: None,
        service_label: None,
        native_web_search: provider.defaults.native_web_search,
        web_search_supported: discovered.web_search_supported,
        web_search_unsupported_reason: (!discovered.web_search_supported).then(|| {
            format!(
                "{} provider model does not support provider-native web search",
                provider.id
            )
        }),
        source: AgentModelSource::Discovered,
        explicit: ModelExplicitFields::default(),
    }
}

fn apply_manual_model_override(existing: &mut ResolvedAgentModel, manual: &ResolvedAgentModel) {
    existing.label = manual.label.clone().or_else(|| existing.label.clone());
    if manual.explicit.max_tokens {
        existing.max_tokens = manual.max_tokens;
    }
    if manual.explicit.temperature {
        existing.temperature = manual.temperature;
    }
    if manual.explicit.reasoning_effort {
        existing.reasoning_effort = manual.reasoning_effort.clone();
    }
    if manual.explicit.service_label {
        existing.service_label = manual.service_label.clone();
    }
    if manual.explicit.native_web_search {
        existing.native_web_search = manual.native_web_search;
    }
    if manual.explicit.web_search_supported {
        existing.web_search_supported = manual.web_search_supported;
        if manual.web_search_supported {
            existing.web_search_unsupported_reason = None;
        }
    }
    if manual.explicit.web_search_unsupported_reason {
        existing.web_search_unsupported_reason = manual.web_search_unsupported_reason.clone();
    }
    existing.source = AgentModelSource::DiscoveredWithOverride;
}

async fn load_from_file(path: PathBuf) -> Result<RigAgentConfig, RigAgentConfigError> {
    let content = read_config_file(&path).await?;
    if is_agents_config(&content)? {
        return rig_config_from_registry_selection(
            parse_agents_config(&content, &path)?.into_registry()?,
            &RigAgentConfigSelection::default(),
        );
    }
    let file: LegacyRigAgentConfigFile =
        toml::from_str(&content).map_err(|e| RigAgentConfigError {
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
    let native_web_search = env_flag("BUDN_AGENT_NATIVE_WEB_SEARCH");

    Some(RigAgentConfig {
        provider_id: "openai".into(),
        provider_kind: AgentProviderKind::OpenAiResponses,
        api_key,
        model,
        timeout_secs,
        max_tokens,
        temperature,
        reasoning_effort,
        service_label: None,
        native_web_search: env::var("BUDN_AGENT_NATIVE_WEB_SEARCH")
            .map(|_| native_web_search)
            .unwrap_or(true),
        anthropic_version: None,
        base_url: None,
    })
}

pub async fn load_rig_agent_config() -> Result<Option<RigAgentConfig>, RigAgentConfigError> {
    if let Ok(path) = env::var("BUDN_AGENT_CONFIG") {
        return load_from_file(PathBuf::from(path)).await.map(Some);
    }
    Ok(load_from_env())
}

pub async fn load_rig_agent_config_with_discovery()
-> Result<Option<RigAgentConfig>, RigAgentConfigError> {
    let Some(registry) = load_agent_provider_registry_with_discovery().await? else {
        return Ok(None);
    };
    rig_config_from_registry_selection(registry, &RigAgentConfigSelection::default()).map(Some)
}

pub async fn load_agent_provider_registry()
-> Result<Option<AgentProviderRegistry>, RigAgentConfigError> {
    if let Ok(path) = env::var("BUDN_AGENT_CONFIG") {
        let path = PathBuf::from(path);
        let content = read_config_file(&path).await?;
        return parse_agents_config(&content, &path)?
            .into_registry()
            .map(Some);
    }
    Ok(load_registry_from_env())
}

pub async fn load_agent_provider_registry_with_discovery()
-> Result<Option<AgentProviderRegistry>, RigAgentConfigError> {
    let Some(mut registry) = load_agent_provider_registry().await? else {
        return Ok(None);
    };
    let providers = registry
        .providers
        .iter()
        .map(discover_and_apply_provider_models)
        .collect::<Vec<_>>();
    let mut resolved = Vec::with_capacity(providers.len());
    for provider in providers {
        resolved.push(provider.await);
    }
    registry.providers = resolved;
    Ok(Some(registry))
}

async fn discover_and_apply_provider_models(
    provider: &ResolvedAgentProvider,
) -> ResolvedAgentProvider {
    let result = discover_provider_models(provider).await;
    apply_provider_model_discovery(provider, result)
}

pub async fn discover_provider_models(
    provider: &ResolvedAgentProvider,
) -> Result<Vec<DiscoveredProviderModel>, String> {
    if !provider.discover_models {
        return Ok(Vec::new());
    }
    let Some(api_key) = provider.api_key.as_deref() else {
        return Err(format!(
            "provider API key env `{}` is not set",
            provider.api_key_env
        ));
    };
    let discovery = async {
        match provider.kind {
            AgentProviderKind::OpenAiResponses => {
                let mut builder = rig::providers::openai::Client::builder().api_key(api_key);
                if let Some(base_url) = provider.base_url.as_deref() {
                    builder = builder.base_url(base_url);
                }
                let client = builder.build().map_err(|error| error.to_string())?;
                let models = client
                    .list_models()
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(models_to_discovered(models.data))
            }
            AgentProviderKind::OpenAiCompletions => {
                let mut builder =
                    rig::providers::openai::CompletionsClient::builder().api_key(api_key);
                if let Some(base_url) = provider.base_url.as_deref() {
                    builder = builder.base_url(base_url);
                }
                let client = builder
                    .build()
                    .map_err(|error| error.to_string())?
                    .responses_api();
                let models = client
                    .list_models()
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(models_to_discovered(models.data))
            }
            AgentProviderKind::Anthropic => {
                let version = provider
                    .anthropic_version
                    .as_deref()
                    .unwrap_or("2023-06-01");
                let mut builder = rig::providers::anthropic::Client::builder()
                    .api_key(api_key.to_owned())
                    .anthropic_version(version);
                if let Some(base_url) = provider.base_url.as_deref() {
                    builder = builder.base_url(base_url);
                }
                let client = builder.build().map_err(|error| error.to_string())?;
                let models = client
                    .list_models()
                    .await
                    .map_err(|error| error.to_string())?;
                Ok(models_to_discovered(models.data))
            }
        }
    };
    time::timeout(Duration::from_secs(10), discovery)
        .await
        .map_err(|_| "provider model discovery timed out".to_owned())?
}

fn models_to_discovered(models: Vec<rig::model::Model>) -> Vec<DiscoveredProviderModel> {
    models
        .into_iter()
        .map(|model| DiscoveredProviderModel {
            label: model.name.unwrap_or_else(|| model.id.clone()),
            id: model.id,
            web_search_supported: true,
        })
        .collect()
}

fn load_registry_from_env() -> Option<AgentProviderRegistry> {
    let config = load_from_env()?;
    let model = ResolvedAgentModel {
        id: config.model.clone(),
        label: Some(config.model.clone()),
        max_tokens: config.max_tokens,
        temperature: config.temperature,
        reasoning_effort: config.reasoning_effort,
        service_label: None,
        native_web_search: config.native_web_search,
        web_search_supported: true,
        web_search_unsupported_reason: None,
        source: AgentModelSource::Manual,
        explicit: ModelExplicitFields::default(),
    };
    Some(registry_from_openai_model(config.api_key, model))
}

fn registry_from_openai_model(api_key: String, model: ResolvedAgentModel) -> AgentProviderRegistry {
    let defaults = AgentConfigDefaults {
        timeout_secs: 120,
        max_tokens: 8192,
        temperature: 0.7,
        native_web_search: true,
        discover_models: true,
    };
    AgentProviderRegistry {
        active_provider_id: "openai".into(),
        active_model_id: model.id.clone(),
        defaults: defaults.clone(),
        providers: vec![ResolvedAgentProvider {
            id: "openai".into(),
            kind: AgentProviderKind::OpenAiResponses,
            api_key: Some(api_key),
            api_key_env: "BUDN_AGENT_OPENAI_API_KEY".into(),
            anthropic_version: None,
            base_url: None,
            discover_models: true,
            model_discovery_status: ModelDiscoveryStatus::NotStarted,
            defaults,
            models: vec![model],
        }],
    }
}

async fn read_config_file(path: &PathBuf) -> Result<String, RigAgentConfigError> {
    fs::read_to_string(path)
        .await
        .map_err(|e| RigAgentConfigError {
            message: format!("Cannot read Rig Agent config file {}: {e}", path.display()),
        })
}

fn parse_agents_config(
    content: &str,
    path: &PathBuf,
) -> Result<AgentsConfigFile, RigAgentConfigError> {
    toml::from_str(content).map_err(|e| RigAgentConfigError {
        message: format!(
            "Cannot parse agents.toml config file {}: {e}",
            path.display()
        ),
    })
}

fn is_agents_config(content: &str) -> Result<bool, RigAgentConfigError> {
    let value = content
        .parse::<toml::Value>()
        .map_err(|e| RigAgentConfigError {
            message: format!("Cannot parse Rig Agent config file: {e}"),
        })?;
    Ok(value.get("providers").is_some() || value.get("active_provider").is_some())
}

pub fn rig_config_from_registry_selection(
    registry: AgentProviderRegistry,
    selection: &RigAgentConfigSelection,
) -> Result<RigAgentConfig, RigAgentConfigError> {
    let provider_id = selection
        .provider_id
        .as_deref()
        .unwrap_or(&registry.active_provider_id);
    let model_id = selection
        .model_id
        .as_deref()
        .unwrap_or(&registry.active_model_id);
    let provider = registry
        .provider(provider_id)
        .ok_or_else(|| RigAgentConfigError {
            message: "active provider is missing".into(),
        })?;
    let model = provider
        .models
        .iter()
        .find(|model| model.id == model_id)
        .ok_or_else(|| RigAgentConfigError {
            message: "active model is missing".into(),
        })?;
    let explicit_model_selection = selection.provider_id.is_some() || selection.model_id.is_some();
    Ok(RigAgentConfig {
        provider_id: provider.id.clone(),
        provider_kind: provider.kind,
        api_key: provider
            .api_key
            .clone()
            .ok_or_else(|| RigAgentConfigError {
                message: format!("provider API key env `{}` is not set", provider.api_key_env),
            })?,
        model: model.id.clone(),
        timeout_secs: registry.defaults.timeout_secs,
        max_tokens: model.max_tokens,
        temperature: model.temperature,
        reasoning_effort: if explicit_model_selection {
            selection.reasoning_effort.clone()
        } else {
            selection
                .reasoning_effort
                .clone()
                .or_else(|| model.reasoning_effort.clone())
        },
        service_label: if explicit_model_selection {
            selection.service_label.clone()
        } else {
            selection
                .service_label
                .clone()
                .or_else(|| model.service_label.clone())
        },
        native_web_search: model.native_web_search && model.web_search_supported,
        anthropic_version: provider.anthropic_version.clone(),
        base_url: provider.base_url.clone(),
    })
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

fn env_flag(key: &str) -> bool {
    env::var(key).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn validate_active_model(registry: &AgentProviderRegistry) -> Result<(), RigAgentConfigError> {
    let provider = registry
        .provider(&registry.active_provider_id)
        .ok_or_else(|| RigAgentConfigError {
            message: format!(
                "active provider `{}` does not exist",
                registry.active_provider_id
            ),
        })?;
    if provider
        .models
        .iter()
        .any(|model| model.id == registry.active_model_id)
    {
        return Ok(());
    }
    config_error(format!(
        "active model `{}` does not exist for provider `{}`",
        registry.active_model_id, registry.active_provider_id
    ))
}

fn validate_active_provider_key(
    registry: &AgentProviderRegistry,
) -> Result<(), RigAgentConfigError> {
    let Some(provider) = registry.active_provider() else {
        return Ok(());
    };
    if provider.api_key.is_some() {
        return Ok(());
    }
    config_error(format!(
        "provider API key env `{}` is not set",
        provider.api_key_env
    ))
}

fn validate_anthropic_version(
    kind: &AgentProviderKind,
    version: Option<&str>,
) -> Result<(), RigAgentConfigError> {
    let Some(version) = version else {
        return Ok(());
    };
    if version.trim().is_empty() {
        return config_error("anthropic_version cannot be empty");
    }
    if kind != &AgentProviderKind::Anthropic {
        return config_error("anthropic_version can only be used by anthropic providers");
    }
    Ok(())
}

fn resolve_anthropic_version(kind: &AgentProviderKind, version: Option<String>) -> Option<String> {
    match kind {
        AgentProviderKind::Anthropic => non_empty(version).or_else(|| Some("2023-06-01".into())),
        AgentProviderKind::OpenAiResponses | AgentProviderKind::OpenAiCompletions => None,
    }
}

fn resolve_provider_base_url(
    kind: &AgentProviderKind,
    base_url: Option<String>,
) -> Result<Option<String>, RigAgentConfigError> {
    let Some(base_url) = non_empty(base_url) else {
        return Ok(None);
    };
    if let Some(raw) = base_url.strip_suffix('#') {
        return require_non_empty("base_url", raw.to_owned()).map(Some);
    }
    match kind {
        AgentProviderKind::OpenAiResponses | AgentProviderKind::OpenAiCompletions => {
            if base_url.ends_with('/') {
                Ok(Some(base_url))
            } else {
                Ok(Some(format!("{base_url}/v1")))
            }
        }
        AgentProviderKind::Anthropic => Ok(Some(base_url)),
    }
}

fn read_api_key_env(key: &str) -> Option<String> {
    env::var(key).ok().filter(|value| !value.trim().is_empty())
}

fn provider_default_web_search_supported(kind: &AgentProviderKind) -> bool {
    match kind {
        AgentProviderKind::OpenAiResponses | AgentProviderKind::Anthropic => true,
        AgentProviderKind::OpenAiCompletions => false,
    }
}

fn web_search_reason(
    model_id: &str,
    supported: bool,
    reason: Option<String>,
) -> Result<Option<String>, RigAgentConfigError> {
    if !supported && reason.as_ref().is_some_and(|value| value.trim().is_empty()) {
        return config_error("web_search_unsupported_reason cannot be empty");
    }
    let reason = non_empty(reason);
    if supported {
        return Ok(reason);
    }
    Ok(Some(reason.unwrap_or_else(|| {
        format!("model `{model_id}` does not support provider-native web search")
    })))
}

fn require_non_empty(label: &str, value: String) -> Result<String, RigAgentConfigError> {
    let value = value.trim().to_owned();
    if value.is_empty() {
        return config_error(format!("{label} cannot be empty"));
    }
    Ok(value)
}

fn config_error<T>(message: impl Into<String>) -> Result<T, RigAgentConfigError> {
    Err(RigAgentConfigError {
        message: message.into(),
    })
}
