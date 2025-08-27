use std::{collections::HashMap, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tracing::{error, info, warn};

#[derive(Clone, Deserialize)]
pub struct LoginResult {
    pub token: String,
}

#[derive(Clone, Deserialize)]
pub struct LoginResponse {
    pub code: u16,
    pub message: String,
    pub value: Option<LoginResult>,
}

pub struct WsClient {
    pub api_host: String,
    pub token: String,
}

impl WsClient {
    pub async fn new(api_host: String) -> anyhow::Result<Self> {
        let client = reqwest::Client::new();
        let mut request_data = HashMap::new();
        request_data.insert("username", "admin");
        request_data.insert("password", "123456");
        let result: LoginResponse = client
            .post(format!("http://{}/v1/login", api_host))
            .json(&request_data)
            .send()
            .await?
            .json()
            .await?;

        if result.code != StatusCode::OK {
            return anyhow::bail!("Login failed: {}", result.message);
        }

        let token = result.value.unwrap().token;

        Ok(Self { api_host, token })
    }

    pub async fn subscribe(&self, shutdown: Arc<tokio::sync::Notify>) {
        tokio::select! {
            _ = shutdown.notified() => {
                info!("Cancel websocket subscribers...");
            },
            _ = self.listen() => {}
        }
    }

    pub async fn reconnect(
        &self,
        mut stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ) -> WebSocketStream<MaybeTlsStream<TcpStream>> {
        let close_timeout = Duration::from_secs(1);
        let retry_interval = Duration::from_secs(5);
        if let Some(stream) = stream.as_mut() {
            if let Err(close_err) = tokio::time::timeout(close_timeout, stream.close(None)).await {
                warn!("Failed to send close frame or timed out: {}", close_err);
            }
        }

        loop {
            info!("Try connect to the ws server...");
            match connect_async(format!("ws://{}/v1/ws/notify", self.api_host)).await {
                Ok((mut stream, _)) => {
                    if let Err(e) = stream
                        .send(Message::Text(format!(
                            "{{\"access_token\":\"{}\",\"action\":\"login\"}}",
                            self.token
                        )))
                        .await
                    {
                        error!("Failed send login to websocket, retry...");
                        tokio::time::sleep(retry_interval).await;
                        continue;
                    }
                    return stream;
                }
                Err(e) => {
                    error!("Failed for connect to ws server: {e}");
                    tokio::time::sleep(retry_interval).await;
                }
            }
        }
    }

    pub async fn listen(&self) {
        let send_timeout = Duration::from_secs(1);
        let mut stream = self.reconnect(None).await;
        info!("Connected to websocket server...");
        loop {
            // 标准websocket心跳间隔30-60s
            match tokio::time::timeout(Duration::from_secs(60), stream.next()).await {
                Ok(Some(Ok(msg))) => match msg {
                    Message::Text(text) => {
                        info!("Received websocket msg: {}", text);
                    }
                    Message::Ping(data) => {
                        info!("Received ping");
                        match tokio::time::timeout(send_timeout, stream.send(Message::Pong(data)))
                            .await
                        {
                            Ok(Ok(_)) => info!("Sent pong"),
                            Ok(Err(e)) => {
                                error!("Failed to send pong: {e}, reconnect...");
                                stream = self.reconnect(Some(stream)).await;
                            }
                            Err(_) => {
                                error!("Timeout sending pong, reconnect...");
                                stream = self.reconnect(Some(stream)).await;
                            }
                        }
                    }
                    Message::Pong(_) => {
                        info!("Received pong");
                    }
                    Message::Close(_) => {
                        info!("Received close frame, reconnect...");
                        stream = self.reconnect(Some(stream)).await;
                    }
                    _ => {}
                },
                Ok(Some(Err(e))) => {
                    error!("Error receiving message: {e}, reconnect...");
                    stream = self.reconnect(Some(stream)).await;
                }
                Ok(None) => {
                    error!("Websocket stream closed, reconnect...");
                    stream = self.reconnect(Some(stream)).await;
                }
                Err(_) => {
                    error!("No response from server for too long, reconnect...");
                    stream = self.reconnect(Some(stream)).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod ws_tests {
    use crate::task::ws::WsClient;

    #[test]
    fn test() {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }
    #[tokio::test]
    async fn test_ws() {
        let ws_client = WsClient::new("192.168.77.14:8080".to_string())
            .await
            .unwrap();
        ws_client.listen().await;
    }
}
