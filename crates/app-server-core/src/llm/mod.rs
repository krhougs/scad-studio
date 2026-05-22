mod config;

#[doc(hidden)]
pub use config::openai_compatible_models_to_discovered;
pub use config::{
    AgentModelSource, AgentProviderKind, AgentProviderRegistry, DiscoveredProviderModel,
    ModelDiscoveryStatus, ResolvedAgentModel, ResolvedAgentProvider, ResolvedWebSearchProvider,
    RigAgentConfig, RigAgentConfigError, RigAgentConfigSelection, WebSearchAuth,
    WebSearchHttpMethod, WebSearchParams, WebSearchResultMap, apply_provider_model_discovery,
    discover_provider_models, load_agent_provider_registry,
    load_agent_provider_registry_with_discovery, load_rig_agent_config,
    load_rig_agent_config_with_discovery, merge_provider_models,
    rig_config_from_registry_selection,
};
