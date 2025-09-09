use rumqttc::v5::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, mqttbytes::QoS};
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;
use tracing::{error, info, warn};

use crate::{config::MqttConfig, handler::Handler};

#[derive(Clone)]
pub struct MqttClient {
    client: AsyncClient,
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
        (Self { client }, eventloop)
    }

    pub async fn publish(&mut self, topic: &'static str, payload: String) {
        if let Err(e) = self
            .client
            .publish(topic, QoS::AtLeastOnce, false, payload.clone())
            .await
        {
            error!(
                "Failed for publish {} to topic: {}, err: {e}",
                payload, topic
            );
        }
    }

    pub async fn subscribe<H: Handler>(
        &self,
        mut eventloop: EventLoop,
        topics: Vec<String>,
        handler: &H,
        shutdown: Arc<Notify>,
    ) -> anyhow::Result<()> {
        tokio::select! {
            _ = shutdown.notified() => {
                info!("Cancel mqtt subscribtions, waitting for mqtt disconnected...");
                match tokio::time::timeout(
                    Duration::from_secs(2),
                    self.client.disconnect()
                ).await {
                    Ok(_) => info!("Mqtt disconnected."),
                    Err(_) => warn!("Force mqtt to disconneted.")
                }

                Ok(())
            }
            result = self.consume(&mut eventloop, topics, handler) => result
        }
    }

    async fn consume<H: Handler>(
        &self,
        eventloop: &mut EventLoop,
        topics: Vec<String>,
        handler: &H,
    ) -> anyhow::Result<()> {
        loop {
            match eventloop.poll().await {
                Ok(event) => match event {
                    Event::Incoming(Incoming::Publish(packet)) => {
                        match std::str::from_utf8(&packet.topic) {
                            Ok(topic) => {
                                if let Err(e) = self.client.ack(&packet).await {
                                    error!("Ack failed: {e}");
                                }
                                if let Err(e) = handler
                                    .proc(topic.to_string(), packet.payload.clone())
                                    .await
                                {
                                    error!("Payload proc failed: {e}");
                                }
                            }
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
}
