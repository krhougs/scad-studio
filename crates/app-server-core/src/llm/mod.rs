mod config;

pub use config::{
    AgentProviderKind, AgentProviderRegistry, DiscoveredProviderModel, RigAgentConfig,
    RigAgentConfigError, load_agent_provider_registry, load_rig_agent_config,
    merge_provider_models,
};
