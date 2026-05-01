mod config;

pub use config::{
    AgentModelSource, AgentProviderKind, AgentProviderRegistry, DiscoveredProviderModel,
    ModelDiscoveryStatus, RigAgentConfig, RigAgentConfigError, apply_provider_model_discovery,
    discover_provider_models, load_agent_provider_registry,
    load_agent_provider_registry_with_discovery, load_rig_agent_config,
    load_rig_agent_config_with_discovery, merge_provider_models,
};
