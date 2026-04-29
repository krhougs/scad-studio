use app_server_host::{WebSocketHostConfig, run_websocket_host};
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
    let url = run_websocket_host(WebSocketHostConfig {
        bind_addr: args.bind_addr,
        workspace_path: args.workspace_path,
    })
    .await?;
    println!("{url}");
    pending::<()>().await;
    Ok(())
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
    if !workspace_path.is_dir() {
        return Err(format!(
            "workspace 路径不存在或不是目录: {}",
            workspace_path.display()
        ));
    }

    Ok(Args {
        bind_addr,
        workspace_path,
    })
}

fn usage() -> String {
    "用法: cargo run -p app-server-host --bin websocket-host -- --workspace <PATH> [--bind 127.0.0.1:39180]".into()
}
