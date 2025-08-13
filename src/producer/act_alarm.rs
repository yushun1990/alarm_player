//! 实际报警生产者，生产方式为从MQTT消费报警信息，
//! 生产的报警写入`real_time` 队列
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
