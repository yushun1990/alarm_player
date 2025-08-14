//! 实际报警生产者，生产方式为从MQTT消费报警信息，
//! 生产的报警写入`real_time` 队列
use std::{sync::Arc, time::Duration};

use rumqttc::v5::{
    AsyncClient, Event, EventLoop, Incoming, MqttOptions,
    mqttbytes::{
        QoS,
        v5::{Packet, Publish},
    },
};
use time::OffsetDateTime;
use tokio::sync::{
    Notify,
    mpsc::{self, Sender},
};
use tracing::{error, info};

use crate::{config::MqttConfig, model::Alarm};

pub struct Producer {
    config: Mqtt,
    real_time_tx: Sender<Alarm>,
    shutdown: Arc<Notify>,
}

impl Producer {
    pub fn new(config: MqttConfig, real_time_tx: Sender<Alarm>, shutdown: Arc<Notify>) -> Self {
        Self {
            config,
            real_time_tx,
            shutdown,
        }
    }

    /// 从MQTT订阅报警消息，写入实时队列
    /// 使用select! 监听程序中断，利用rumqttc 自动重连机制，重连后必须重新订阅（实测
    /// 发现当前平台 `clean_start=false`不起作用，因此配置中要把`clean_start`设置
    /// 为 true）
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

    async fn send_alarm(sender: Sender<Alarm>, client: AsyncClient, packet: Publish) {
        info!("Received alarm: {:?}", &packet.payload);
        let mut alarm = match serde_json::from_slice::<Alarm>(&packet.payload) {
            Ok(alarm) => alarm,
            Err(e) => {
                error!(
                    "Deserialized to alarm faield: {e}; source: {:?}",
                    &packet.payload
                );
                Self::ack(&client, &packet).await;
                return;
            }
        };

        alarm.received_time = OffsetDateTime::now_utc();
        alarm.is_new = true;

        info!("Received alarm: {:?}", &alarm);

        if let Err(e) = sender.send(alarm).await {
            error!("Send to real time failed: {e}");
        }

        Self::ack(&client, &packet).await;
    }

    async fn ack(client: &AsyncClient, packet: &Publish) {
        if let Err(e) = client.ack(packet).await {
            error!("Ack alarm failed: {e}");
        }
    }

    async fn listen(&self, client: &AsyncClient, mut eventloop: EventLoop) -> anyhow::Result<()> {
        loop {
            match eventloop.poll().await {
                Ok(event) => {
                    match event {
                        Event::Incoming(Incoming::Publish(packet)) => {
                            let client = client.clone();
                            let sender = self.real_time_tx.clone();
                            tokio::spawn(async move {
                                Self::send_alarm(sender, client, packet).await;
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
