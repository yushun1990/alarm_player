use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use bytes::Bytes;
use rumqttc::v5::{
    AsyncClient, Event, EventLoop, Incoming, MqttOptions,
    mqttbytes::{QoS, v5::Publish},
};
use tokio::sync::Notify;
use tracing::{error, info};

use crate::config::MqttConfig;

/// 消息处理器
#[async_trait]
pub trait Produce: Send + Sync {
    /// topic 匹配
    async fn mat(&self, topic: &str) -> bool;
    /// 消息处理
    async fn proc(&self, payload: Bytes) -> anyhow::Result<()>;
}

pub struct MqttClient {
    client: AsyncClient,
    produces: Vec<Box<dyn Produce>>,
}

impl MqttClient {
    pub fn new(config: MqttConfig) -> (Self, EventLoop) {
        let mut options = MqttOptions::new(config.client_id(), config.broker(), config.port());
        options
            .set_credentials(config.username(), config.password())
            .set_keep_alive(Duration::from_secs(config.keep_alive().into()))
            .set_clean_start(config.clean_session())
            .set_manual_acks(true);

        let (client, eventloop) = AsyncClient::new(options, 10);
        (
            Self {
                client,
                produces: Vec::new(),
            },
            eventloop,
        )
    }

    pub fn client(&self) -> AsyncClient {
        self.client.clone()
    }

    pub fn produce<T: Produce + 'static>(mut self, p: T) -> Self {
        self.produces.push(Box::new(p));
        self
    }

    pub async fn subscribe(
        &self,
        mut eventloop: EventLoop,
        topics: Vec<String>,
        shutdown: Arc<Notify>,
    ) -> anyhow::Result<()> {
        tokio::select! {
            _ = shutdown.notified() => {
                info!("Cancel mqtt subscribtions...");
                self.client.disconnect().await?;
                info!("mqtt disconnected...");
                Ok(())
            }
            result = self.consume(&mut eventloop, topics) => result
        }
    }

    async fn consume(&self, eventloop: &mut EventLoop, topics: Vec<String>) -> anyhow::Result<()> {
        loop {
            match eventloop.poll().await {
                Ok(event) => match event {
                    Event::Incoming(Incoming::Publish(packet)) => {
                        match std::str::from_utf8(&packet.topic) {
                            Ok(topic) => match topic.split('/').last() {
                                Some(topic) => {
                                    self.distribute(topic, &packet).await;
                                    if let Err(e) = self.client.ack(&packet).await {
                                        error!("Ack failed: {e}");
                                    }
                                }
                                None => error!("Invalid topic: {:?}", packet.topic),
                            },
                            Err(e) => error!("Topic extract failed: {e}"),
                        }
                    }
                    Event::Incoming(Incoming::ConnAck(_)) => {
                        info!("MQTT connected, subscribe to broker...");
                        for topic in &topics {
                            self.client
                                .subscribe(topic.to_string(), QoS::AtLeastOnce)
                                .await?;
                        }
                    }
                    _ => continue,
                },
                Err(e) => {
                    error!("MQTT error: {e}, auto reconnect...");
                }
            }
        }
    }

    async fn distribute(&self, topic: &str, packet: &Publish) {
        for p in &self.produces {
            if p.mat(topic).await {
                info!("topic: {topic}, payload: {:?}", packet.payload);
                if let Err(e) = p.proc(packet.payload.clone()).await {
                    error!("Packet proc failed: {e}");
                }
                return;
            }
        }
        error!("No producer was matched to topic: {topic}");
    }
}
