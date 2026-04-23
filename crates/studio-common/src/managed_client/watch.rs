use app_server_protocol::{PathHandle, SubscriptionId, WatchSubscribeRequest};
use std::collections::HashSet;

use super::types::WatchEventPayload;

pub const DEFAULT_WATCH_THROTTLE_MS: u32 = 150;

#[derive(Debug, Clone)]
pub struct WatchRegistryEntry {
    #[allow(dead_code)]
    pub request: WatchSubscribeRequest,
    pub throttle_ms: u32,
    pub subscription_id: Option<SubscriptionId>,
    pub envelope_bytes: Vec<u8>,
    pub accumulator: WatchAccumulator,
    pub awaiting_resubscribe: bool,
}

#[derive(Debug, Clone, Default)]
pub struct WatchAccumulator {
    window_start_ms: Option<u64>,
    changed_paths: HashSet<PathHandle>,
}

impl WatchAccumulator {
    pub fn is_empty(&self) -> bool {
        self.changed_paths.is_empty()
    }

    pub fn window_start_ms(&self) -> Option<u64> {
        self.window_start_ms
    }

    pub fn ingest(&mut self, now_ms: u64, paths: impl IntoIterator<Item = PathHandle>) {
        if self.window_start_ms.is_none() {
            self.window_start_ms = Some(now_ms);
        }
        for path in paths {
            self.changed_paths.insert(path);
        }
    }

    pub fn take_window(
        &mut self,
        subscription_id: SubscriptionId,
        window_end_ms: u64,
    ) -> Option<WatchEventPayload> {
        let window_start_ms = self.window_start_ms.take()?;
        if self.changed_paths.is_empty() {
            return None;
        }
        let changed_paths = std::mem::take(&mut self.changed_paths)
            .into_iter()
            .collect::<Vec<_>>();
        Some(WatchEventPayload::Changed {
            subscription_id,
            window_start_ms,
            window_end_ms,
            changed_paths,
        })
    }
}
