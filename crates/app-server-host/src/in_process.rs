use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;

pub const GUI_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbortDecision {
    CleanExit,
    Abort,
}

pub trait AbortStrategy {
    fn on_timeout(&self) -> AbortDecision;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinThenAbort {
    pub timeout: Duration,
}

impl Default for JoinThenAbort {
    fn default() -> Self {
        Self {
            timeout: GUI_SHUTDOWN_TIMEOUT,
        }
    }
}

impl AbortStrategy for JoinThenAbort {
    fn on_timeout(&self) -> AbortDecision {
        AbortDecision::Abort
    }
}

pub fn evaluate_shutdown(all_tasks_joined: bool, strategy: &impl AbortStrategy) -> AbortDecision {
    if all_tasks_joined {
        AbortDecision::CleanExit
    } else {
        strategy.on_timeout()
    }
}

#[derive(Debug, Default, Clone)]
pub struct InProcessHost {
    workspace_path: Option<PathBuf>,
    workspace_tx: Option<UnboundedSender<PathBuf>>,
}

impl InProcessHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_workspace_binding(workspace_tx: UnboundedSender<PathBuf>) -> Self {
        Self {
            workspace_path: None,
            workspace_tx: Some(workspace_tx),
        }
    }

    pub fn current_workspace(&self) -> Option<&Path> {
        self.workspace_path.as_deref()
    }

    pub fn rebind_workspace(&mut self, workspace_path: PathBuf) {
        if let Some(tx) = self.workspace_tx.as_ref() {
            let _ = tx.send(workspace_path.clone());
        }
        self.workspace_path = Some(workspace_path);
    }
}
