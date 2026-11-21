//! One JSON-RPC message per text frame; the provider's existing request IDs,
//! events, approvals and questions are independent of its local/remote transport.
use super::{
    transport::{Messages, Writer},
    CodexError,
};
use crate::remote::native::Access;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

pub(super) fn open(access: &Access, url: &str) -> Result<(Writer, Messages), CodexError> {
    let (mut ws, _) = access
        .io
        .block_on(async {
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                tokio_tungstenite::connect_async_with_config(
                    url,
                    Some(
                        tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
                            .max_message_size(Some(32 * 1024 * 1024)),
                    ),
                    true,
                ),
            )
            .await
        })
        .map_err(|_| CodexError::Transport("VM Codex WebSocket handshake timed out.".into()))?
        .map_err(|e| CodexError::Transport(format!("VM Codex WebSocket handshake failed: {e}")))?;
    let (tx, mut rx) = mpsc::channel::<String>(128);
    let (stop, mut closing) = watch::channel(false);
    let (messages, input) = std::sync::mpsc::sync_channel(512);
    let live = access.live.clone();
    access.io.spawn(async move {
        loop {
            tokio::select! {
                _ = closing.changed() => break,
                msg = rx.recv() => match msg {
                    Some(s) if live.load(std::sync::atomic::Ordering::Acquire) => {
                        if ws.send(Message::Text(s.into())).await.is_err() { break }
                    }
                    _ => break,
                },
                msg = ws.next() => match msg {
                    Some(Ok(Message::Text(s))) => if messages.try_send(Ok(s.to_string())).is_err() { break },
                    Some(Ok(Message::Ping(_))) => if ws.flush().await.is_err() { break },
                    Some(Ok(Message::Pong(_))) => {},
                    _ => break,
                },
            }
        }
    });
    Ok((
        Writer::WebSocket(tx, stop, access.live.clone()),
        Box::new(input.into_iter()),
    ))
}
