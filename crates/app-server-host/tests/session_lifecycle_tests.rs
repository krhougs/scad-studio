use app_server_core::{ChildTerminator, terminate_child_with};
use app_server_host::{
    AbortDecision, ClientTransport, HostSession, JoinThenAbort, MpscTransportAdapter,
    evaluate_shutdown,
};
use app_server_protocol::{
    PreviewRequestKind, ProtocolError, ProtocolErrorCode, ProtocolVersionRange, RequestId,
    ServerCapabilities, SessionToken, SubscriptionId, WorkspaceCurrentResponse, WorkspaceId,
};
use app_server_transport::{ClientEnvelope, ServerEnvelope};
use std::io;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[test]
fn explicit_cancel_returns_cancelled_error() {
    let (mut transport, harness) = MpscTransportAdapter::pair();
    transport.cancel(RequestId(2), RequestId(1)).unwrap();
    let Some(ClientEnvelope::Request(request)) = harness.pop_client_message() else {
        panic!("expected cancel request")
    };
    assert_eq!(request.request_id, RequestId(2));
    harness
        .inject_protocol_error(
            RequestId(2),
            ProtocolError::new(ProtocolErrorCode::Cancelled, "cancelled"),
        )
        .unwrap();
    let Some(ServerEnvelope::Response(response)) = transport.next_server_message().unwrap() else {
        panic!("expected response")
    };
    assert_eq!(
        response.result.unwrap_err().code,
        ProtocolErrorCode::Cancelled
    );
}

#[test]
fn disconnect_abandons_in_flight_tasks() {
    let now = Instant::now();
    let mut session = build_session();
    session.bind_workspace(WorkspaceCurrentResponse {
        workspace_id: WorkspaceId::new("ws"),
        root_name: "workspace".into(),
    });
    session.track_request(RequestId(1));
    session.track_subscription(SubscriptionId("sub-1".into()));
    session.disconnect(now, Duration::from_secs(30));

    assert_eq!(session.in_flight_count(), 0);
    assert_eq!(session.subscription_count(), 0);
    assert!(session.can_reclaim(now + Duration::from_secs(5)));
    assert!(session.workspace().is_some());
}

#[test]
fn disconnect_cancels_subscriptions_no_auto_resume() {
    let now = Instant::now();
    let mut session = build_session();
    session.track_subscription(SubscriptionId("sub-1".into()));
    session.disconnect(now, Duration::from_secs(30));
    assert_eq!(session.subscription_count(), 0);
    assert!(session.can_reclaim(now + Duration::from_secs(10)));
    assert_eq!(session.subscription_count(), 0);
}

#[test]
fn child_terminate_on_cancel() {
    let invoked = Arc::new(AtomicBool::new(false));
    let mut child = Command::new("python3")
        .arg("-c")
        .arg("import time; time.sleep(5)")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let terminator = TestTerminator {
        invoked: Arc::clone(&invoked),
    };
    terminate_child_with(&mut child, &terminator).unwrap();
    let _ = child.wait();
    assert!(invoked.load(Ordering::SeqCst));
}

#[test]
fn gui_shutdown_5s_join_then_abort_strategy() {
    let strategy = JoinThenAbort::default();
    assert_eq!(evaluate_shutdown(true, &strategy), AbortDecision::CleanExit);
    assert_eq!(evaluate_shutdown(false, &strategy), AbortDecision::Abort);
}

#[derive(Debug)]
struct TestTerminator {
    invoked: Arc<AtomicBool>,
}

impl ChildTerminator for TestTerminator {
    fn terminate(&self, child: &mut Child) -> io::Result<()> {
        self.invoked.store(true, Ordering::SeqCst);
        child.kill()
    }
}

fn build_session() -> HostSession {
    HostSession::new(
        SessionToken("session-1".into()),
        ServerCapabilities {
            protocol_version: ProtocolVersionRange::new(1, 2),
            reconnect_window_ms: 30_000,
            supports_watch: true,
            supported_preview_kinds: vec![PreviewRequestKind::GeometryArtifact],
            supports_session_reclaim: true,
        },
    )
}
