use std::collections::HashSet;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
struct ClientCommand {
    action: String,
    channel: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct ServerResponse {
    event: String,
    channel: Option<String>,
    payload: Option<serde_json::Value>,
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx_broadcast = state.tx_realtime.subscribe();
    let mut subscriptions: HashSet<String> = HashSet::new();

    info!("WebSocket client connected");

    loop {
        tokio::select! {
            // Receive from WebSocket client
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        debug!("WS received text: {}", text);
                        if let Ok(cmd) = serde_json::from_str::<ClientCommand>(&text) {
                            match cmd.action.as_str() {
                                "subscribe" => {
                                    if let Some(ch) = cmd.channel {
                                        subscriptions.insert(ch.clone());
                                        let resp = serde_json::json!({
                                            "event": "subscribed",
                                            "channel": ch
                                        });
                                        let _ = sender.send(Message::Text(resp.to_string().into())).await;
                                    }
                                }
                                "unsubscribe" => {
                                    if let Some(ch) = cmd.channel {
                                        subscriptions.remove(&ch);
                                        let resp = serde_json::json!({
                                            "event": "unsubscribed",
                                            "channel": ch
                                        });
                                        let _ = sender.send(Message::Text(resp.to_string().into())).await;
                                    }
                                }
                                "ping" => {
                                    let resp = serde_json::json!({ "event": "pong" });
                                    let _ = sender.send(Message::Text(resp.to_string().into())).await;
                                }
                                _ => {}
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        if sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    Some(Err(e)) => {
                        error!("WS error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }

            // Receive from AppState broadcast channel
            broadcast_msg = rx_broadcast.recv() => {
                match broadcast_msg {
                    Ok(msg) => {
                        // Check if client is subscribed to this channel or global "*"
                        if subscriptions.contains(&msg.channel) || subscriptions.contains("*") {
                            let payload = serde_json::json!({
                                "channel": msg.channel,
                                "event": msg.event,
                                "payload": msg.payload
                            });
                            if sender.send(Message::Text(payload.to_string().into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        debug!("WS broadcast lagged, skipped {} messages", skipped);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    info!("WebSocket client disconnected");
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/ws", get(ws_handler))
}
