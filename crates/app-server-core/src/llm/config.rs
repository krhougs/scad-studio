use rig::{
    http_client::{self, HttpClientExt},
    prelude::ModelListingClient,
};
use serde::Deserialize;
use std::{collections::HashMap, collections::HashSet, env, fmt, path::PathBuf, time::Duration};
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
    pub active_web_search_id: Option<String>,
    pub web_search_providers: Vec<ResolvedWebSearchProvider>,
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

    pub fn active_web_search_provider(&self) -> Option<&ResolvedWebSearchProvider> {
        let id = self.active_web_search_id.as_deref()?;
        self.web_search_providers.iter().find(|p| p.id == id)
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

#[derive(Deserialize)]
struct OpenAiCompatibleModelList {
    data: Vec<OpenAiCompatibleModelEntry>,
}

#[derive(Deserialize)]
struct OpenAiCompatibleModelEntry {
    id: String,
    name: Option<String>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebSearchHttpMethod {
    Get,
    Post,
}

#[derive(Clone)]
pub enum WebSearchAuth {
    None,
    Header {
        header: String,
        prefix: Option<String>,
        api_key: String,
    },
    QueryParam {
        param: String,
        api_key: String,
    },
}

impl fmt::Debug for WebSearchAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None => write!(f, "None"),
            Self::Header { header, prefix, .. } => f
                .debug_struct("Header")
                .field("header", header)
                .field("prefix", prefix)
                .field("api_key", &"***")
                .finish(),
            Self::QueryParam { param, .. } => f
                .debug_struct("QueryParam")
                .field("param", param)
                .field("api_key", &"***")
                .finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum WebSearchParams {
    QueryPairs(Vec<(String, String)>),
    JsonObject(serde_json::Map<String, serde_json::Value>),
}

#[derive(Clone, Debug)]
pub struct WebSearchResultMap {
    pub results_path: String,
    pub title: String,
    pub url: String,
    pub snippet: String,
    pub date: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone)]
pub struct ResolvedWebSearchProvider {
    pub id: String,
    pub endpoint: String,
    pub method: WebSearchHttpMethod,
    pub query_key: String,
    pub top_k_param: Option<String>,
    pub auth: WebSearchAuth,
    pub max_result_chars: Option<usize>,
    pub user_agent: Option<String>,
    pub params: WebSearchParams,
    pub result_map: WebSearchResultMap,
}

impl fmt::Debug for ResolvedWebSearchProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedWebSearchProvider")
            .field("id", &self.id)
            .field("endpoint", &self.endpoint)
            .field("method", &self.method)
            .field("query_key", &self.query_key)
            .field("auth", &self.auth)
            .finish()
    }
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
    active_web_search: Option<String>,
    #[serde(default)]
    web_search_providers: Vec<WebSearchProviderFile>,
}

#[derive(Deserialize)]
struct WebSearchProviderFile {
    id: String,
    endpoint: String,
    method: Option<String>,
    query_key: String,
    top_k_param: Option<String>,
    api_key: Option<String>,
    auth_header: Option<String>,
    auth_prefix: Option<String>,
    auth_query_param: Option<String>,
    max_result_chars: Option<usize>,
    user_agent: Option<String>,
    #[serde(default)]
    params: HashMap<String, serde_json::Value>,
    result_map: WebSearchResultMapFile,
}

#[derive(Deserialize)]
struct WebSearchResultMapFile {
    results: String,
    title: String,
    url: String,
    snippet: String,
    date: Option<String>,
    source: Option<String>,
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
    api_key: Option<String>,
    api_key_env: Option<String>,
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
        let active_web_search_id = non_empty(self.active_web_search);
        let web_search_providers =
            resolve_web_search_providers(self.web_search_providers)?;
        let registry = AgentProviderRegistry {
            active_provider_id: require_non_empty("active_provider", self.active_provider)?,
            active_model_id: require_non_empty("active_model", self.active_model)?,
            defaults,
            providers,
            active_web_search_id,
            web_search_providers,
        };
        validate_active_model(&registry)?;
        validate_active_provider_key(&registry)?;
        validate_active_web_search(&registry)?;
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
    let api_key_env = non_empty(provider.api_key_env);
    let api_key =
        non_empty(provider.api_key).or_else(|| api_key_env.as_deref().and_then(read_api_key_env));
    let models = resolve_models(provider.models, &provider.kind, defaults)?;
    let base_url = resolve_provider_base_url(&provider.kind, provider.base_url)?;
    Ok(ResolvedAgentProvider {
        id,
        kind: provider.kind.clone(),
        api_key,
        api_key_env: api_key_env.unwrap_or_default(),
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
    let web_search_supported =
        discovered.web_search_supported && provider_default_web_search_supported(&provider.kind);
    ResolvedAgentModel {
        id: discovered.id,
        label: Some(discovered.label),
        max_tokens: provider.defaults.max_tokens,
        temperature: provider.defaults.temperature,
        reasoning_effort: None,
        service_label: None,
        native_web_search: provider.defaults.native_web_search,
        web_search_supported,
        web_search_unsupported_reason: (!web_search_supported).then(|| {
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

fn resolve_web_search_providers(
    providers: Vec<WebSearchProviderFile>,
) -> Result<Vec<ResolvedWebSearchProvider>, RigAgentConfigError> {
    let mut seen = HashSet::new();
    providers
        .into_iter()
        .map(|p| resolve_web_search_provider(p, &mut seen))
        .collect()
}

fn resolve_web_search_provider(
    provider: WebSearchProviderFile,
    seen: &mut HashSet<String>,
) -> Result<ResolvedWebSearchProvider, RigAgentConfigError> {
    let id = require_non_empty("web_search_providers.id", provider.id)?;
    if !seen.insert(id.clone()) {
        return config_error(format!("duplicate web_search_providers id `{id}`"));
    }
    let endpoint = require_non_empty("web_search_providers.endpoint", provider.endpoint)?;
    let method = resolve_web_search_method(provider.method)?;
    let query_key = require_non_empty("web_search_providers.query_key", provider.query_key)?;
    let auth = resolve_web_search_auth(
        &id,
        provider.api_key,
        provider.auth_header,
        provider.auth_prefix,
        provider.auth_query_param,
    )?;
    validate_web_search_result_map(&id, &provider.result_map)?;
    let params = resolve_web_search_params(&method, provider.params);
    let result_map = WebSearchResultMap {
        results_path: provider.result_map.results,
        title: provider.result_map.title,
        url: provider.result_map.url,
        snippet: provider.result_map.snippet,
        date: non_empty(provider.result_map.date),
        source: non_empty(provider.result_map.source),
    };
    Ok(ResolvedWebSearchProvider {
        id,
        endpoint,
        method,
        query_key,
        top_k_param: non_empty(provider.top_k_param),
        auth,
        max_result_chars: provider.max_result_chars,
        user_agent: non_empty(provider.user_agent),
        params,
        result_map,
    })
}

fn resolve_web_search_method(
    method: Option<String>,
) -> Result<WebSearchHttpMethod, RigAgentConfigError> {
    match method.as_deref().unwrap_or("GET") {
        s if s.eq_ignore_ascii_case("GET") => Ok(WebSearchHttpMethod::Get),
        s if s.eq_ignore_ascii_case("POST") => Ok(WebSearchHttpMethod::Post),
        other => config_error(format!(
            "web_search_providers.method must be GET or POST, got `{other}`"
        )),
    }
}

fn resolve_web_search_auth(
    provider_id: &str,
    api_key: Option<String>,
    auth_header: Option<String>,
    auth_prefix: Option<String>,
    auth_query_param: Option<String>,
) -> Result<WebSearchAuth, RigAgentConfigError> {
    let has_header = auth_header.is_some();
    let has_query_param = auth_query_param.is_some();
    if has_header && has_query_param {
        return config_error(format!(
            "web_search_providers `{provider_id}`: auth_header and auth_query_param are mutually exclusive"
        ));
    }
    let api_key = non_empty(api_key);
    if (has_header || has_query_param) && api_key.is_none() {
        return config_error(format!(
            "web_search_providers `{provider_id}`: auth requires api_key"
        ));
    }
    if let Some(header) = auth_header {
        let header = require_non_empty("auth_header", header)?;
        return Ok(WebSearchAuth::Header {
            header,
            prefix: auth_prefix.filter(|s| !s.is_empty()),
            api_key: api_key.unwrap(),
        });
    }
    if let Some(param) = auth_query_param {
        let param = require_non_empty("auth_query_param", param)?;
        return Ok(WebSearchAuth::QueryParam {
            param,
            api_key: api_key.unwrap(),
        });
    }
    Ok(WebSearchAuth::None)
}

fn resolve_web_search_params(
    method: &WebSearchHttpMethod,
    params: HashMap<String, serde_json::Value>,
) -> WebSearchParams {
    match method {
        WebSearchHttpMethod::Get => {
            let pairs = params
                .into_iter()
                .map(|(k, v)| (k, json_value_to_query_string(&v)))
                .collect();
            WebSearchParams::QueryPairs(pairs)
        }
        WebSearchHttpMethod::Post => {
            let map = params.into_iter().collect();
            WebSearchParams::JsonObject(map)
        }
    }
}

fn json_value_to_query_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn validate_web_search_result_map(
    provider_id: &str,
    map: &WebSearchResultMapFile,
) -> Result<(), RigAgentConfigError> {
    if map.results.trim().is_empty() {
        return config_error(format!(
            "web_search_providers `{provider_id}`: result_map.results cannot be empty"
        ));
    }
    if map.title.trim().is_empty() {
        return config_error(format!(
            "web_search_providers `{provider_id}`: result_map.title cannot be empty"
        ));
    }
    if map.url.trim().is_empty() {
        return config_error(format!(
            "web_search_providers `{provider_id}`: result_map.url cannot be empty"
        ));
    }
    if map.snippet.trim().is_empty() {
        return config_error(format!(
            "web_search_providers `{provider_id}`: result_map.snippet cannot be empty"
        ));
    }
    Ok(())
}

fn validate_active_web_search(
    registry: &AgentProviderRegistry,
) -> Result<(), RigAgentConfigError> {
    let Some(id) = registry.active_web_search_id.as_deref() else {
        return Ok(());
    };
    if registry.web_search_providers.iter().any(|p| p.id == id) {
        return Ok(());
    }
    config_error(format!(
        "active_web_search `{id}` does not match any web_search_providers"
    ))
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
    if let Err(error) = &result {
        log::error!(
            "[agent model discovery] provider `{}` kind `{}` base_url `{}` failed: {}",
            provider.id,
            provider.kind.as_str(),
            provider.base_url.as_deref().unwrap_or("<default>"),
            error
        );
    }
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
                list_openai_compatible_models(api_key, provider.base_url.as_deref()).await
            }
            AgentProviderKind::OpenAiCompletions => {
                list_openai_compatible_models(api_key, provider.base_url.as_deref()).await
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

async fn list_openai_compatible_models(
    api_key: &str,
    base_url: Option<&str>,
) -> Result<Vec<DiscoveredProviderModel>, String> {
    let path = "/models";
    let mut builder = rig::providers::openai::Client::builder().api_key(api_key);
    if let Some(base_url) = base_url {
        builder = builder.base_url(base_url);
    }
    let client = builder.build().map_err(|error| error.to_string())?;
    let req = client
        .get(path)
        .map_err(|error| error.to_string())?
        .body(http_client::NoBody)
        .map_err(|error| error.to_string())?;
    let response = client
        .send::<_, Vec<u8>>(req)
        .await
        .map_err(|error| error.to_string())?;
    let body = response
        .into_body()
        .await
        .map_err(|error| error.to_string())?;
    openai_compatible_models_to_discovered(&body, path)
}

pub fn openai_compatible_models_to_discovered(
    body: &[u8],
    path: &str,
) -> Result<Vec<DiscoveredProviderModel>, String> {
    let response: OpenAiCompatibleModelList = serde_json::from_slice(body).map_err(|error| {
        format!(
            "Parse error: provider=OpenAI path={path} parse_error={error} body_bytes={} response_body_preview:\n{}",
            body.len(),
            response_body_preview(body)
        )
    })?;
    Ok(response
        .data
        .into_iter()
        .map(|model| {
            let label = model.name.unwrap_or_else(|| model.id.clone());
            DiscoveredProviderModel {
                id: model.id,
                label,
                web_search_supported: true,
            }
        })
        .collect())
}

fn response_body_preview(body: &[u8]) -> String {
    const LIMIT: usize = 2048;
    let preview = if body.len() > LIMIT {
        &body[..LIMIT]
    } else {
        body
    };
    String::from_utf8_lossy(preview).into_owned()
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
        active_web_search_id: None,
        web_search_providers: Vec::new(),
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
    if provider.api_key_env.is_empty() {
        return config_error(format!(
            "provider `{}` must set api_key or api_key_env",
            provider.id
        ));
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
