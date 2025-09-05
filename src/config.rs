use clap::Parser;
use config::{Environment, File};
use serde::Deserialize;
use tracing::error;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(short, long, default_value = "config.toml")]
    pub config: String,
    #[arg(short, long)]
    pub localization: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DbConfig {
    connection: Option<String>,
    max_conns: Option<u32>,
    min_conns: Option<u32>,
    conn_timeout_millis: Option<u64>,
    idle_timeout_millis: Option<u64>,
    logging_level: Option<String>,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            connection: Some(
                "postgres://postgres:BHzpdmYyyAV1*GHm@127.0.0.1:5432/AwingIB2?currentSchema=public"
                    .into(),
            ),
            max_conns: Default::default(),
            min_conns: Default::default(),
            conn_timeout_millis: Default::default(),
            idle_timeout_millis: Default::default(),
            logging_level: Some("info".to_string()),
        }
    }
}

impl DbConfig {
    pub fn connection(&self) -> String {
        if let Some(connection) = self.connection.clone() {
            connection
        } else {
            Self::default().connection.unwrap()
        }
    }

    pub fn max_conns(&self) -> Option<u32> {
        self.max_conns
    }

    pub fn min_conns(&self) -> Option<u32> {
        self.min_conns
    }

    pub fn conn_timeout_millis(&self) -> Option<u64> {
        self.conn_timeout_millis
    }

    pub fn idle_timeout_millis(&self) -> Option<u64> {
        self.idle_timeout_millis
    }

    pub fn logging_level(&self) -> Option<String> {
        self.logging_level.clone()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TracingConfig {
    level: Option<String>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            level: Some("info".into()),
        }
    }
}

impl TracingConfig {
    pub fn level(&self) -> String {
        if let Some(level) = self.level.clone() {
            level
        } else {
            Self::default().level.unwrap()
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct MqttConfig {
    client_id: Option<String>,
    broker: Option<String>,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    keep_alive: Option<u16>,
    clean_session: Option<bool>,
    topic_alarms: Option<Vec<String>>,
    topic_test: Option<String>,
    topic_speeker: Option<String>,
}

impl MqttConfig {
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

#[derive(Debug, Clone, Deserialize)]
pub struct AlarmConfig {
    // 报警状态检查间隔
    asc_interval_secs: Option<u64>,
    // 循环队列间隔
    cycle_interval_secs: Option<u64>,
    // 报警播放间隔
    play_interval_secs: Option<u64>,
    // 播放延时
    play_delay_secs: Option<u64>,
    // 默认测试报警播放时长
    default_test_play_duration: Option<u64>,
    // 报警测试音乐最小时长
    test_min_duration: Option<u64>,
    // 报警音乐播放最小时长
    alarm_min_duration: Option<u64>,
    // 语音最小播放时长
    speech_min_duration: Option<u64>,
    // 报警测试调度为空时检测周期
    empty_schedule_secs: Option<u64>,
    // 报警初始化接口地址
    init_url: Option<String>,
    // 默认语言
    default_language: Option<String>,
}

impl Default for AlarmConfig {
    fn default() -> Self {
        Self {
            asc_interval_secs: Some(5),
            cycle_interval_secs: Some(5),
            play_interval_secs: Some(5),
            play_delay_secs: Some(20),
            default_test_play_duration: Some(60),
            test_min_duration: Some(30),
            alarm_min_duration: Some(15),
            speech_min_duration: Some(10),
            empty_schedule_secs: Some(5),
            init_url: Some(
                "http://127.0.0.1/api/IB/alarm-info/current-alarm-info-page-list-with-no-auth"
                    .into(),
            ),
            default_language: Some("zh_cn".into()),
        }
    }
}

impl AlarmConfig {
    pub fn asc_interval_secs(&self) -> u64 {
        if let Some(asc_interval_secs) = self.asc_interval_secs {
            asc_interval_secs
        } else {
            Self::default().asc_interval_secs.unwrap()
        }
    }

    pub fn cycle_interval_secs(&self) -> u64 {
        if let Some(cycle_interval_secs) = self.cycle_interval_secs {
            cycle_interval_secs
        } else {
            Self::default().cycle_interval_secs.unwrap()
        }
    }

    pub fn play_interval_secs(&self) -> u64 {
        if let Some(play_interval_secs) = self.play_interval_secs {
            play_interval_secs
        } else {
            Self::default().play_interval_secs.unwrap()
        }
    }

    pub fn play_delay_secs(&self) -> u64 {
        if let Some(play_delay_secs) = self.play_delay_secs {
            play_delay_secs
        } else {
            Self::default().play_delay_secs.unwrap()
        }
    }

    pub fn default_test_play_duration(&self) -> u64 {
        if let Some(default_test_play_duration) = self.default_test_play_duration {
            default_test_play_duration
        } else {
            Self::default().default_test_play_duration.unwrap()
        }
    }

    pub fn empty_schedule_secs(&self) -> u64 {
        if let Some(empty_schedule_secs) = self.empty_schedule_secs {
            empty_schedule_secs
        } else {
            Self::default().empty_schedule_secs.unwrap()
        }
    }

    pub fn default_langauge(&self) -> String {
        if let Some(default_language) = self.default_language.clone() {
            default_language
        } else {
            Self::default().default_language.unwrap()
        }
    }

    pub fn test_min_duration(&self) -> u64 {
        if let Some(duration) = self.test_min_duration.clone() {
            duration
        } else {
            Self::default().test_min_duration.unwrap()
        }
    }

    pub fn alarm_min_duration(&self) -> u64 {
        if let Some(duration) = self.alarm_min_duration.clone() {
            duration
        } else {
            Self::default().alarm_min_duration.unwrap()
        }
    }

    pub fn speech_min_duration(&self) -> u64 {
        if let Some(duration) = self.speech_min_duration.clone() {
            duration
        } else {
            Self::default().speech_min_duration.unwrap()
        }
    }

    pub fn init_url(&self) -> String {
        if let Some(init_url) = self.init_url.clone() {
            init_url
        } else {
            Self::default().init_url.unwrap()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueueConfig {
    pub real_time_size: Option<usize>,
    pub player_size: Option<usize>,
    pub cycle_size: Option<usize>,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            real_time_size: Some(100),
            player_size: Some(10),
            cycle_size: Some(10),
        }
    }
}

impl QueueConfig {
    pub fn real_time_size(&self) -> usize {
        if let Some(real_time_size) = self.real_time_size {
            real_time_size
        } else {
            Self::default().real_time_size.unwrap()
        }
    }

    pub fn player_size(&self) -> usize {
        if let Some(player_size) = self.player_size {
            player_size
        } else {
            Self::default().player_size.unwrap()
        }
    }

    pub fn cycle_size(&self) -> usize {
        if let Some(cycle_size) = self.cycle_size {
            cycle_size
        } else {
            Self::default().cycle_size.unwrap()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecorderConfig {
    // 报警录音存储路径
    record_storage_path: Option<String>,
    // 报警录音连接存储路径
    record_link_path: Option<String>,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            record_storage_path: Some("/data/alarm_player/records".to_string()),
            record_link_path: Some("/data/alarm_player/link".to_string()),
        }
    }
}

impl RecorderConfig {
    pub fn record_storage_path(&self) -> String {
        if let Some(record_storage_path) = self.record_storage_path.clone() {
            record_storage_path
        } else {
            Self::default().record_storage_path.unwrap()
        }
    }

    pub fn record_link_path(&self) -> String {
        if let Some(record_link_path) = self.record_link_path.clone() {
            record_link_path
        } else {
            Self::default().record_link_path.unwrap()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundboxConfig {
    // 报警播放音频文件
    alarm_media_path: Option<String>,
    // 测试报警音频文件
    test_media_path: Option<String>,
}

impl Default for SoundboxConfig {
    fn default() -> Self {
        Self {
            alarm_media_path: Some("./resource/alarm.wav".to_string()),
            test_media_path: Some("./resource/test_alarm.wav".to_string()),
        }
    }
}

impl SoundboxConfig {
    pub fn alarm_media_path(&self) -> String {
        if let Some(alarm_media_path) = self.alarm_media_path.clone() {
            alarm_media_path
        } else {
            Self::default().alarm_media_path.unwrap()
        }
    }

    pub fn test_media_path(&self) -> String {
        if let Some(test_media_path) = self.test_media_path.clone() {
            test_media_path
        } else {
            Self::default().test_media_path.unwrap()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub enum PlayMode {
    #[serde(rename = "music")]
    Music,
    #[serde(rename = "tts")]
    Tts,
}

impl Default for PlayMode {
    fn default() -> Self {
        PlayMode::Tts
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct SoundpostConfig {
    api_host: Option<String>,
    api_login_token: Option<String>,
    alarm_media_url: Option<String>,
    test_media_url: Option<String>,
    play_mode: Option<PlayMode>,
}

impl Default for SoundpostConfig {
    fn default() -> Self {
        Self {
            api_host: Some("127.0.0.1:8080".into()),
            api_login_token: Some("YWRtaW46YWRtaW5fYXBpX2tleQ==".into()),
            alarm_media_url: Some("http://host.docker.internal:80/NewAlarm.wav".into()),
            test_media_url: Some("http://host.docker.internal:80/TestAlarm.wav".into()),
            play_mode: Some(PlayMode::Tts),
        }
    }
}

impl SoundpostConfig {
    pub fn api_host(&self) -> String {
        if let Some(api_addr) = self.api_host.clone() {
            api_addr
        } else {
            Self::default().api_host.unwrap()
        }
    }

    pub fn api_login_token(&self) -> String {
        if let Some(api_login_token) = self.api_login_token.clone() {
            api_login_token
        } else {
            Self::default().api_login_token.unwrap()
        }
    }

    pub fn alarm_media_url(&self) -> String {
        if let Some(alarm_media_url) = self.alarm_media_url.clone() {
            alarm_media_url
        } else {
            Self::default().alarm_media_url.unwrap()
        }
    }

    pub fn test_media_url(&self) -> String {
        if let Some(test_media_url) = self.test_media_url.clone() {
            test_media_url
        } else {
            Self::default().test_media_url.unwrap()
        }
    }

    pub fn play_mode(&self) -> PlayMode {
        if let Some(play_mode) = self.play_mode.clone() {
            play_mode
        } else {
            Self::default().play_mode.unwrap()
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub database: DbConfig,
    #[serde(default)]
    pub tracing: TracingConfig,
    #[serde(default)]
    pub mqtt: MqttConfig,
    #[serde(default)]
    pub alarm: AlarmConfig,
    #[serde(default)]
    pub queue: QueueConfig,
    #[serde(default)]
    pub soundbox: SoundboxConfig,
    #[serde(default)]
    pub soundpost: SoundpostConfig,
    #[serde(default)]
    pub recorder: RecorderConfig,
}

impl Config {
    pub fn new(location: &str) -> anyhow::Result<Self> {
        dotenvy::dotenv().ok();

        let config = match config::Config::builder()
            .add_source(File::with_name(location))
            .add_source(
                Environment::with_prefix("AP")
                    .separator("_")
                    .prefix_separator("__"),
            )
            .build()
        {
            Ok(config) => config,
            Err(e) => {
                error!("Config error: {e}; using the default config.");
                return Ok(Config::default());
            }
        };

        let config = config.try_deserialize()?;

        Ok(config)
    }
}
