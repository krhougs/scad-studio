use app_server_protocol::{
    PathHandle, RequestId, SubscriptionId, WatchChangedEvent, WatchSubscribeRequest,
    WatchSubscriptionAck, WatchUnsubscribeRequest, WorkspaceId,
};
use studio_common::{DirectoryWatchLifecycle, WatchLifecycleRequest};

#[test]
fn entering_root_requests_subscribe_and_matching_push_requests_refresh() {
    let mut lifecycle = DirectoryWatchLifecycle::default();

    let request = lifecycle
        .enter_directory(None)
        .expect("entering root should request subscribe");
    assert_eq!(
        request,
        WatchLifecycleRequest::Subscribe(WatchSubscribeRequest { directory: None })
    );

    lifecycle.record_sent_request(request, RequestId(1));
    assert_eq!(
        lifecycle.handle_watch_subscribed(
            RequestId(1),
            WatchSubscriptionAck {
                subscription_id: subscription("root-sub"),
            },
        ),
        None
    );

    assert_eq!(
        lifecycle.refresh_directory_for(&WatchChangedEvent {
            subscription_id: subscription("root-sub"),
            changed_paths: vec![path_handle(["README.md"])],
        }),
        Some(None)
    );
}

#[test]
fn switching_directory_unsubscribes_old_subscription_before_subscribing_new_target() {
    let mut lifecycle = DirectoryWatchLifecycle::default();

    let root_request = lifecycle
        .enter_directory(None)
        .expect("root should subscribe");
    lifecycle.record_sent_request(root_request, RequestId(1));
    lifecycle.handle_watch_subscribed(
        RequestId(1),
        WatchSubscriptionAck {
            subscription_id: subscription("root-sub"),
        },
    );

    let examples = path_handle(["examples"]);
    let unsubscribe = lifecycle
        .enter_directory(Some(examples.clone()))
        .expect("switching target should unsubscribe old subscription first");
    assert_eq!(
        unsubscribe,
        WatchLifecycleRequest::Unsubscribe(WatchUnsubscribeRequest {
            subscription_id: subscription("root-sub"),
        })
    );

    lifecycle.record_sent_request(unsubscribe, RequestId(2));
    let subscribe_new = lifecycle
        .handle_watch_unsubscribed(
            RequestId(2),
            WatchSubscriptionAck {
                subscription_id: subscription("root-sub"),
            },
        )
        .expect("unsubscribe ack should trigger subscribe for new target");
    assert_eq!(
        subscribe_new,
        WatchLifecycleRequest::Subscribe(WatchSubscribeRequest {
            directory: Some(examples.clone()),
        })
    );

    lifecycle.record_sent_request(subscribe_new, RequestId(3));
    lifecycle.handle_watch_subscribed(
        RequestId(3),
        WatchSubscriptionAck {
            subscription_id: subscription("examples-sub"),
        },
    );

    assert_eq!(
        lifecycle.refresh_directory_for(&WatchChangedEvent {
            subscription_id: subscription("root-sub"),
            changed_paths: vec![path_handle(["README.md"])],
        }),
        None
    );
    assert_eq!(
        lifecycle.refresh_directory_for(&WatchChangedEvent {
            subscription_id: subscription("examples-sub"),
            changed_paths: vec![path_handle(["examples", "notes.txt"])],
        }),
        Some(Some(examples))
    );
}

#[test]
fn switching_target_while_subscribe_is_in_flight_reconciles_after_ack() {
    let mut lifecycle = DirectoryWatchLifecycle::default();

    let root_request = lifecycle
        .enter_directory(None)
        .expect("root should subscribe");
    lifecycle.record_sent_request(root_request, RequestId(1));

    assert_eq!(
        lifecycle.enter_directory(Some(path_handle(["examples"]))),
        None,
        "new target should wait until in-flight subscribe settles"
    );

    let unsubscribe = lifecycle
        .handle_watch_subscribed(
            RequestId(1),
            WatchSubscriptionAck {
                subscription_id: subscription("root-sub"),
            },
        )
        .expect("stale root subscribe should immediately transition toward desired target");
    assert_eq!(
        unsubscribe,
        WatchLifecycleRequest::Unsubscribe(WatchUnsubscribeRequest {
            subscription_id: subscription("root-sub"),
        })
    );
}

fn path_handle<const N: usize>(segments: [&str; N]) -> PathHandle {
    let segments = segments
        .into_iter()
        .map(|segment| segment.to_string())
        .collect::<Vec<String>>();
    PathHandle::new(WorkspaceId::new("workspace"), segments).expect("path handle should be valid")
}

fn subscription(value: &str) -> SubscriptionId {
    SubscriptionId(value.to_string())
}
