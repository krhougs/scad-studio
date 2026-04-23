//! `NullTransport` —— 用于 `ManagedClient<T>` 的 trait 形式参数。
//!
//! `ManagedClient` 内部仅把 `T` 作为泛型形参持有，实际上不 invoke trait 方法
//! （outbound 字节走 `next_outbound` 队列，inbound 走 `receive_inbound`）。
//! 所以这里只需提供一个同步、全返回 Ok 的 stub；任何调用路径都不会触发 panic。

use app_server_protocol::{
    CapabilityHandshakeRequest, ClientRequestEnvelope, RequestId, WatchSubscribeRequest,
    WatchUnsubscribeRequest,
};
use studio_common::{AppServerTransportError, AppServerTransportEvent, AppServerTransportPort};

#[derive(Debug, Default)]
pub(crate) struct NullTransport;

impl AppServerTransportPort for NullTransport {
    fn handshake(
        &mut self,
        _request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn reconnect(
        &mut self,
        _request: CapabilityHandshakeRequest,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn request(&mut self, _request: ClientRequestEnvelope) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn subscribe(
        &mut self,
        _request_id: RequestId,
        _request: WatchSubscribeRequest,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn unsubscribe(
        &mut self,
        _request_id: RequestId,
        _request: WatchUnsubscribeRequest,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn cancel(
        &mut self,
        _request_id: RequestId,
        _target_request_id: RequestId,
    ) -> Result<(), AppServerTransportError> {
        Ok(())
    }

    fn poll_server_event(
        &mut self,
    ) -> Result<Option<AppServerTransportEvent>, AppServerTransportError> {
        Ok(None)
    }

    fn close(&mut self) -> Result<(), AppServerTransportError> {
        Ok(())
    }
}
