use app_server_protocol::{PathHandle, PreviewArtifact, PreviewReadyResponse, PreviewRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
enum PreviewPhase {
    Idle,
    Pending,
    Ready,
    Unsupported,
    Skipped,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewState {
    target: Option<PathHandle>,
    current_request: Option<PreviewRequest>,
    last_result: Option<PreviewReadyResponse>,
    last_error: Option<String>,
    phase: PreviewPhase,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            target: None,
            current_request: None,
            last_result: None,
            last_error: None,
            phase: PreviewPhase::Idle,
        }
    }
}

impl PreviewState {
    pub fn target(&self) -> Option<&PathHandle> {
        self.target.as_ref()
    }

    pub fn current_request(&self) -> Option<&PreviewRequest> {
        self.current_request.as_ref()
    }

    pub fn last_result(&self) -> Option<&PreviewReadyResponse> {
        self.last_result.as_ref()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn start(&mut self, request: PreviewRequest) {
        self.target = Some(request.source.clone());
        self.current_request = Some(request);
        self.last_result = None;
        self.last_error = None;
        self.phase = PreviewPhase::Pending;
    }

    pub fn complete(&mut self, ready: PreviewReadyResponse) {
        self.current_request = None;
        let phase = match ready.artifact {
            PreviewArtifact::Mesh(_) | PreviewArtifact::ThreeMf(_) | PreviewArtifact::Stl(_) => {
                self.last_error = None;
                PreviewPhase::Ready
            }
            ref artifact => {
                self.last_error = Some(format!("unexpected artifact: {artifact:?}"));
                PreviewPhase::Unsupported
            }
        };
        self.last_result = Some(ready);
        self.phase = phase;
    }

    pub fn skip(&mut self, message: impl Into<String>) {
        self.target = None;
        self.current_request = None;
        self.last_result = None;
        self.last_error = Some(message.into());
        self.phase = PreviewPhase::Skipped;
    }

    pub fn fail(&mut self, message: impl Into<String>) {
        self.current_request = None;
        self.last_result = None;
        self.last_error = Some(message.into());
        self.phase = PreviewPhase::Error;
    }

    pub fn status_label(&self) -> &'static str {
        match self.phase {
            PreviewPhase::Idle => "preview idle",
            PreviewPhase::Pending => "preview pending",
            PreviewPhase::Ready => "preview ready",
            PreviewPhase::Unsupported => "preview unsupported",
            PreviewPhase::Skipped => "preview skipped",
            PreviewPhase::Error => "preview error",
        }
    }

    pub fn summary_label(&self) -> String {
        match self.phase {
            PreviewPhase::Idle => "No preview requested yet.".into(),
            PreviewPhase::Pending => pending_summary(self.current_request.as_ref()),
            PreviewPhase::Ready => ready_summary(self.last_result.as_ref()),
            PreviewPhase::Unsupported | PreviewPhase::Skipped | PreviewPhase::Error => self
                .last_error
                .clone()
                .unwrap_or_else(|| "Preview status unavailable.".into()),
        }
    }
}

fn pending_summary(request: Option<&PreviewRequest>) -> String {
    match request.map(|request| request.kind) {
        Some(app_server_protocol::PreviewRequestKind::RenderedImage) => {
            "Waiting for rendered image response…".into()
        }
        _ => "Waiting for mesh response…".into(),
    }
}

fn ready_summary(result: Option<&PreviewReadyResponse>) -> String {
    match result.map(|result| &result.artifact) {
        Some(PreviewArtifact::Mesh(mesh)) => {
            format!(
                "vertices: {} · indices: {} · triangles: {}",
                mesh.positions.len(),
                mesh.indices.len(),
                mesh.indices.len() / 3
            )
        }
        Some(PreviewArtifact::ThreeMf(artifact)) => {
            format!("3mf bytes: {}", artifact.bytes.len())
        }
        Some(PreviewArtifact::Stl(artifact)) => {
            format!("stl bytes: {}", artifact.bytes.len())
        }
        _ => "Preview result unavailable.".into(),
    }
}
