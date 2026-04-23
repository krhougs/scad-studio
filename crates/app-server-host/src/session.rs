use app_server_protocol::{
    PathHandle, RequestId, ServerCapabilities, SessionToken, SubscriptionId,
    WorkspaceCurrentResponse,
};
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct HostSession {
    token: SessionToken,
    workspace: Option<WorkspaceCurrentResponse>,
    capabilities: ServerCapabilities,
    issued_handles: HashSet<PathHandle>,
    in_flight: HashSet<RequestId>,
    subscriptions: HashSet<SubscriptionId>,
    reclaim_deadline: Option<Instant>,
}

impl HostSession {
    pub fn new(token: SessionToken, capabilities: ServerCapabilities) -> Self {
        Self {
            token,
            workspace: None,
            capabilities,
            issued_handles: HashSet::new(),
            in_flight: HashSet::new(),
            subscriptions: HashSet::new(),
            reclaim_deadline: None,
        }
    }

    pub fn token(&self) -> &SessionToken {
        &self.token
    }

    pub fn capabilities(&self) -> &ServerCapabilities {
        &self.capabilities
    }

    pub fn replace_capabilities(&mut self, capabilities: ServerCapabilities) {
        self.capabilities = capabilities;
    }

    pub fn workspace(&self) -> Option<&WorkspaceCurrentResponse> {
        self.workspace.as_ref()
    }

    pub fn issued_handles(&self) -> &HashSet<PathHandle> {
        &self.issued_handles
    }

    pub fn in_flight_count(&self) -> usize {
        self.in_flight.len()
    }

    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    pub fn bind_workspace(&mut self, workspace: WorkspaceCurrentResponse) {
        self.workspace = Some(workspace);
    }

    pub fn issue_handle(&mut self, handle: PathHandle) {
        self.issued_handles.insert(handle);
    }

    pub fn track_request(&mut self, request_id: RequestId) {
        self.in_flight.insert(request_id);
    }

    pub fn complete_request(&mut self, request_id: &RequestId) {
        self.in_flight.remove(request_id);
    }

    pub fn cancel_request(&mut self, request_id: &RequestId) -> bool {
        self.in_flight.remove(request_id)
    }

    pub fn track_subscription(&mut self, subscription_id: SubscriptionId) {
        self.subscriptions.insert(subscription_id);
    }

    pub fn untrack_subscription(&mut self, subscription_id: &SubscriptionId) {
        self.subscriptions.remove(subscription_id);
    }

    pub fn disconnect(&mut self, now: Instant, reclaim_window: Duration) {
        self.in_flight.clear();
        self.subscriptions.clear();
        self.reclaim_deadline = Some(now + reclaim_window);
    }

    pub fn can_reclaim(&self, now: Instant) -> bool {
        self.reclaim_deadline
            .is_some_and(|deadline| now <= deadline)
    }
}
