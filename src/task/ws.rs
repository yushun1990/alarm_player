use std::{collections::HashMap, sync::Arc, time::Duration};

use futures_util::{SinkExt, StreamExt};
use reqwest::StatusCode;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
use tracing::{error, info};

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

    pub async fn subscribe(&mut self, shutdown: Arc<tokio::sync::Notify>) {}

    pub async fn listen(&self) -> anyhow::Result<()> {
        let mut timeouted = false;
        let (mut stream, _) = connect_async(format!("ws://{}/v1/ws/notify", self.api_host)).await?;
        info!("Connected to websocket server...");
        loop {
            match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
                Ok(Some(Ok(msg))) => match msg {
                    Message::Text(text) => {
                        timeouted = false;
                        info!("Received websocket msg: {}", text);
                    }
                    Message::Ping(data) => {
                        timeouted = false;
                        info!("Received ping");
                        match tokio::time::timeout(
                            Duration::from_secs(1),
                            stream.send(Message::Pong(data)),
                        )
                        .await
                        {
                            Ok(Ok(_)) => info!("Sent pong"),
                            Ok(Err(e)) => {
                                error!("Failed to send pong: {e}, reconnect...");
                                return Err(e.into());
                            }
                            Err(_) => {
                                error!("Timeout sending pong, reconnect...");
                                return anyhow::bail!("Pong send timeout");
                            }
                        }
                    }
                    Message::Pong(_) => {
                        info!("Received pong");
                        timeouted = false;
                    }
                    Message::Close(_) => {
                        info!("Received close frame, reconnect...");
                        return Ok(());
                    }
                    _ => {}
                },
                Ok(Some(Err(e))) => {
                    error!("Error receiving message: {e}, reconnect...");
                    return Err(e.into());
                }
                Ok(None) => {
                    error!("Websocket stream closed, reconnect...");
                    return anyhow::bail!("Websocket stream closed.");
                }
                Err(_) => {
                    error!("No response from server for too long, reconnect...");
                    match tokio::time::timeout(
                        Duration::from_secs(1),
                        stream.send(Message::Text("heartbeat".to_string())),
                    )
                    .await
                    {
                        Ok(Ok(_)) => info!("Sent heartbeat"),
                        Ok(Err(e)) => {
                            error!("Failed to send heartbeat: {e}");
                            return Err(e.into());
                        }
                        Err(_) => {
                            error!("Send heartbeat timeout!");
                            return anyhow::bail!("Send heartbeat timeout!");
                        }
                    }

                    return anyhow::bail!("No response from server for too long, disconnecting");
                }
            }
        }
    }
}
