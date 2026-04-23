use app_server_protocol::{
    PathHandle, RequestId, SubscriptionId, WatchChangedEvent, WatchSubscribeRequest,
    WatchSubscriptionAck, WatchUnsubscribeRequest,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchLifecycleRequest {
    Subscribe(WatchSubscribeRequest),
    Unsubscribe(WatchUnsubscribeRequest),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirectoryWatchLifecycle {
    desired_directory: Option<Option<PathHandle>>,
    active_subscription: Option<ActiveSubscription>,
    pending_request: Option<PendingRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveSubscription {
    directory: Option<PathHandle>,
    subscription_id: SubscriptionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PendingRequest {
    Subscribe {
        request_id: RequestId,
        directory: Option<PathHandle>,
    },
    Unsubscribe {
        request_id: RequestId,
        subscription_id: SubscriptionId,
    },
}

impl DirectoryWatchLifecycle {
    pub fn enter_directory(
        &mut self,
        directory: Option<PathHandle>,
    ) -> Option<WatchLifecycleRequest> {
        self.desired_directory = Some(directory);
        self.next_request()
    }

    pub fn record_sent_request(&mut self, request: WatchLifecycleRequest, request_id: RequestId) {
        self.pending_request = Some(match request {
            WatchLifecycleRequest::Subscribe(WatchSubscribeRequest { directory }) => {
                PendingRequest::Subscribe {
                    request_id,
                    directory,
                }
            }
            WatchLifecycleRequest::Unsubscribe(WatchUnsubscribeRequest { subscription_id }) => {
                PendingRequest::Unsubscribe {
                    request_id,
                    subscription_id,
                }
            }
        });
    }

    pub fn handle_watch_subscribed(
        &mut self,
        request_id: RequestId,
        ack: WatchSubscriptionAck,
    ) -> Option<WatchLifecycleRequest> {
        let Some(PendingRequest::Subscribe {
            request_id: pending_request_id,
            directory,
        }) = self.pending_request.clone()
        else {
            return None;
        };
        if pending_request_id != request_id {
            return None;
        }

        self.pending_request = None;
        self.active_subscription = Some(ActiveSubscription {
            directory,
            subscription_id: ack.subscription_id,
        });
        self.next_request()
    }

    pub fn handle_watch_unsubscribed(
        &mut self,
        request_id: RequestId,
        ack: WatchSubscriptionAck,
    ) -> Option<WatchLifecycleRequest> {
        let Some(PendingRequest::Unsubscribe {
            request_id: pending_request_id,
            subscription_id,
        }) = self.pending_request.clone()
        else {
            return None;
        };
        if pending_request_id != request_id {
            return None;
        }

        self.pending_request = None;
        if self
            .active_subscription
            .as_ref()
            .is_some_and(|active| active.subscription_id == subscription_id)
            && ack.subscription_id == subscription_id
        {
            self.active_subscription = None;
        }
        self.next_request()
    }

    pub fn refresh_directory_for(&self, event: &WatchChangedEvent) -> Option<Option<PathHandle>> {
        let active = self.active_subscription.as_ref()?;
        if event.changed_paths.is_empty() || event.subscription_id != active.subscription_id {
            return None;
        }
        Some(active.directory.clone())
    }

    fn next_request(&self) -> Option<WatchLifecycleRequest> {
        if self.pending_request.is_some() {
            return None;
        }

        let desired_directory = self.desired_directory.clone()?;
        match self.active_subscription.as_ref() {
            Some(active) if active.directory == desired_directory => None,
            Some(active) => Some(WatchLifecycleRequest::Unsubscribe(
                WatchUnsubscribeRequest {
                    subscription_id: active.subscription_id.clone(),
                },
            )),
            None => Some(WatchLifecycleRequest::Subscribe(WatchSubscribeRequest {
                directory: desired_directory,
            })),
        }
    }
}
