use app_server_host::{
    WebSocketHostConfig, cadquery_python_path, run_websocket_host,
    verify_cadquery_runner_environment,
};
use std::future::pending;
use std::path::PathBuf;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    env_logger::init();
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let args = parse_args(std::env::args().skip(1).collect())?;
    let python = cadquery_python_path();
    verify_cadquery_runner_environment(&python).await?;
    verify_agent_provider_config().await?;
    if !tokio::fs::metadata(&args.workspace_path)
        .await
        .is_ok_and(|metadata| metadata.is_dir())
    {
        return Err(format!(
            "workspace 路径不存在或不是目录: {}",
            args.workspace_path.display()
        ));
    }
    let url = run_websocket_host(WebSocketHostConfig {
        bind_addr: args.bind_addr,
        workspace_path: args.workspace_path,
    })
    .await?;
    println!("{url}");
    pending::<()>().await;
    Ok(())
}

async fn verify_agent_provider_config() -> Result<(), String> {
    let registry = app_server_core::llm::load_agent_provider_registry_with_discovery()
        .await
        .map_err(|error| agent_provider_startup_error(error.message))?
        .ok_or_else(|| {
            agent_provider_startup_error(
                "missing provider config: set BUDN_AGENT_CONFIG=agents.toml or configure BUDN_AGENT_OPENAI_API_KEY",
            )
        })?;
    if registry.providers.is_empty() {
        return Err(agent_provider_startup_error(
            "provider config loaded but contains no providers",
        ));
    }
    log::info!(
        "[agent provider config] loaded active provider `{}` model `{}` with {} provider(s)",
        registry.active_provider_id,
        registry.active_model_id,
        registry.providers.len()
    );
    Ok(())
}

fn agent_provider_startup_error(message: impl AsRef<str>) -> String {
    let message = message.as_ref();
    log::error!("[agent provider config] startup rejected: {message}");
    format!(
        "\n\
        ================================================================================\n\
        AGENT PROVIDER CONFIG ERROR\n\
        websocket-host refused to start because Agent provider config is invalid.\n\
        {message}\n\
        Expected: BUDN_AGENT_CONFIG=agents.toml with at least one provider, active_provider,\n\
        active_model, and either api_key or api_key_env for the active provider.\n\
        ================================================================================\n"
    )
}

struct Args {
    bind_addr: String,
    workspace_path: PathBuf,
}

fn parse_args(args: Vec<String>) -> Result<Args, String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(usage());
    }

    let mut bind_addr = "127.0.0.1:39180".to_string();
    let mut workspace_path = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--bind" => {
                index += 1;
                bind_addr = args.get(index).cloned().ok_or_else(usage)?;
            }
            "--workspace" => {
                index += 1;
                workspace_path = Some(PathBuf::from(args.get(index).cloned().ok_or_else(usage)?));
            }
            other => return Err(format!("未知参数: {other}\n\n{}", usage())),
        }
        index += 1;
    }

    let workspace_path = workspace_path.ok_or_else(usage)?;
    Ok(Args {
        bind_addr,
        workspace_path,
    })
}

fn usage() -> String {
    "用法: cargo run -p app-server-host --bin websocket-host -- --workspace <PATH> [--bind 127.0.0.1:39180]".into()
}
