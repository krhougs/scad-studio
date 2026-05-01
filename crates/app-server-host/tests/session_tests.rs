use app_server_host::HostSession;
use app_server_protocol::{
    PathHandle, PreviewRequestKind, ProtocolVersionRange, ServerCapabilities, SessionToken,
    SubscriptionId, WorkspaceCurrentResponse, WorkspaceId,
};
use std::time::{Duration, Instant};

#[test]
fn session_token_reclaim_within_window() {
    let now = Instant::now();
    let mut session = build_session();
    session.bind_workspace(WorkspaceCurrentResponse {
        workspace_id: WorkspaceId::new("ws"),
        root_name: "workspace".into(),
    });
    session.issue_handle(PathHandle::new(WorkspaceId::new("ws"), ["src", "main.scad"]).unwrap());
    session.track_subscription(SubscriptionId("sub-1".into()));
    session.disconnect(now, Duration::from_secs(30));

    assert!(session.can_reclaim(now + Duration::from_secs(10)));
    assert_eq!(session.workspace().unwrap().root_name, "workspace");
    assert_eq!(session.issued_handles().len(), 1);
    assert_eq!(session.subscription_count(), 0);
}

#[test]
fn session_token_invalid_after_window() {
    let now = Instant::now();
    let mut session = build_session();
    session.disconnect(now, Duration::from_secs(30));

    assert!(!session.can_reclaim(now + Duration::from_secs(31)));
}

#[test]
fn disconnect_clears_in_flight_and_subscriptions_but_keeps_context() {
    let now = Instant::now();
    let mut session = build_session();
    session.bind_workspace(WorkspaceCurrentResponse {
        workspace_id: WorkspaceId::new("ws"),
        root_name: "workspace".into(),
    });
    session.issue_handle(PathHandle::new(WorkspaceId::new("ws"), ["docs", "guide.md"]).unwrap());
    session.track_request(app_server_protocol::RequestId(5));
    session.track_subscription(SubscriptionId("sub-1".into()));
    session.disconnect(now, Duration::from_secs(30));

    assert_eq!(session.in_flight_count(), 0);
    assert_eq!(session.subscription_count(), 0);
    assert_eq!(session.issued_handles().len(), 1);
    assert!(session.workspace().is_some());
}

fn build_session() -> HostSession {
    HostSession::new(
        SessionToken("session-1".into()),
        ServerCapabilities {
            protocol_version: ProtocolVersionRange::new(4, 4),
            reconnect_window_ms: 30_000,
            supports_watch: true,
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
            supports_session_reclaim: true,
            cadquery: true,
            agent: false,
            selection_sync: false,
            llm_configured: false,
            agent_provider: None,
        },
    )
}
