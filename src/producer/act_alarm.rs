use std::{sync::Arc, time::Duration};

use rumqttc::v5::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, mqttbytes::QoS};
use tokio::sync::{Notify, mpsc};
use tracing::{error, info};

use crate::config::{Alarm, Mqtt};

pub struct Producer {
    config: Mqtt,
    alarm_tx: mpsc::Sender<Alarm>,
    shutdown: Arc<Notify>,
}

impl Producer {
    pub fn new(config: Mqtt, alarm_tx: mpsc::Sender<Alarm>, shutdown: Arc<Notify>) -> Self {
        Self {
            config,
            alarm_tx,
            shutdown,
        }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        info!("Begin to run....");
        let mut options = MqttOptions::new(
            self.config.client_id(),
            self.config.broker(),
            self.config.port(),
        );
        options
            .set_credentials(self.config.username(), self.config.password())
            .set_keep_alive(Duration::from_secs(self.config.keep_alive().into()))
            .set_clean_start(self.config.clean_session())
            .set_manual_acks(true);

        let (client, eventloop) = AsyncClient::new(options, 10);
        for topic in self.config.topic_alarms() {
            client.subscribe(topic.clone(), QoS::AtLeastOnce).await?;
            info!("subscribed to {topic}");
        }

        tokio::select! {
            _ = self.shutdown.notified() => {
                info!("Shutdown alarm consumer...");
                client.disconnect().await?;
                return Ok(())
            },
            result = self.listen(&client, eventloop) => result
        }
    }

    async fn listen(&self, client: &AsyncClient, mut eventloop: EventLoop) -> anyhow::Result<()> {
        loop {
            match eventloop.poll().await {
                Ok(event) => {
                    match event {
                        Event::Incoming(Incoming::Publish(packet)) => {
                            let client = client.clone();
                            tokio::spawn(async move {
                                info!("Received alarm: {:?}", &packet.payload);
                                if let Err(e) = client.ack(&packet).await {
                                    error!("Ack alarm failed: {e}");
                                }
                            });
                        }
                        Event::Incoming(Incoming::ConnAck(_)) => {
                            info!("Mqtt reconneced!");
                            if self.config.clean_session() {
                                // if `clean_sesison == true`, we need resubscribe the
                                // topic when connection reconnected.
                                info!("Re subscribe to broker...");
                                for topic in self.config.topic_alarms() {
                                    client.subscribe(topic, QoS::AtLeastOnce).await?;
                                }
                            }
                        }
                        _ => continue,
                    }
                }
                Err(e) => {
                    error!("MQTT error: {e}, auto reconnect...");
                }
            }
        }
    }
}
