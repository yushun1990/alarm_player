use config::{Environment, File};
use serde::Deserialize;

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Database {
    pub connection: Option<String>,
    pub max_conns: Option<u32>,
    pub min_conns: Option<u32>,
    pub conn_timeout_millis: Option<u64>,
    pub idle_timeout_millis: Option<u64>,
    pub enable_logging: Option<bool>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Tracing {
    pub level: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Mqtt {
    pub client_id: Option<String>,
    pub broker: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub keep_alive: Option<u16>,
    pub clean_session: Option<bool>,
    pub topic_alarms: Option<Vec<String>>,
    pub topic_test: Option<String>,
    pub topic_speeker: Option<String>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Alarm {
    // 报警状态检查间隔
    pub asc_interval_secs: Option<u32>,
    // 报警播放间隔
    pub play_interval_secs: Option<u32>,
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Config {
    pub database: Database,
    pub tracing: Tracing,
    pub mqtt: Mqtt,
    pub alarm: Alarm,
}

impl Config {
    pub fn new(location: &str) -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let config = config::Config::builder()
            .add_source(File::with_name(location))
            .add_source(
                Environment::with_prefix("AP")
                    .separator("_")
                    .prefix_separator("__"),
            )
            .build()?;

        let config = config.try_deserialize()?;

        Ok(config)
    }
}

impl Mqtt {
    pub fn client_id(&self) -> String {
        if let Some(client_id) = self.client_id.clone() {
            client_id
        } else {
            "CLIENT_ALARM_PLAYER".into()
        }
    }

    pub fn broker(&self) -> String {
        if let Some(broker) = self.broker.clone() {
            broker
        } else {
            "127.0.0.1".into()
        }
    }

    pub fn port(&self) -> u16 {
        if let Some(port) = self.port.clone() {
            port
        } else {
            1883
        }
    }

    pub fn keep_alive(&self) -> u16 {
        if let Some(keep_alive) = self.keep_alive.clone() {
            keep_alive
        } else {
            5
        }
    }

    pub fn clean_session(&self) -> bool {
        if let Some(clean_session) = self.clean_session.clone() {
            clean_session
        } else {
            false
        }
    }

    pub fn topic_alarms(&self) -> Vec<String> {
        if let Some(topic_alarm) = self.topic_alarms.clone() {
            topic_alarm
        } else {
            [
                "$share/ap/+/+/alarm".into(),
                "$share/ap/+/+/repub_alarms".into(),
            ]
            .into()
        }
    }

    pub fn topic_test(&self) -> String {
        if let Some(topic_test) = self.topic_test.clone() {
            topic_test
        } else {
            "/ap/config/test".into()
        }
    }

    pub fn topic_speeker(&self) -> String {
        if let Some(topic_speeker) = self.topic_speeker.clone() {
            topic_speeker
        } else {
            "/ap/status/speeker".into()
        }
    }

    pub fn username(&self) -> String {
        if let Some(username) = self.username.clone() {
            username
        } else {
            "admin".into()
        }
    }

    pub fn password(&self) -> String {
        if let Some(password) = self.password.clone() {
            password
        } else {
            "BHzpdmYyyAV1*GHm".into()
        }
    }
}
