use crate::model::{
    farm_config_info, sound_column_config, sys_house, test_alarm_config, test_alarm_play_record,
};
use crate::player::PlayCancelType;
use crate::util::rfc3339_time;
use chrono::Utc;
use cron::Schedule;
use sea_orm::{ConnectOptions, Database, DatabaseConnection};
use serde::Deserialize;
use std::fs;
use std::str::FromStr;
use std::{collections::HashMap, time::Duration};
use time::{OffsetDateTime, PrimitiveDateTime};
use tracing::{error, info, warn};
use tracing_log::log::LevelFilter;

use crate::{config::DbConfig, model::Alarm, player::PlayResultType};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmsInitResp {
    pub total_count: u32,
    pub items: Vec<AlarmInitRespItem>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmInitRespItem {
    pub farm_id: Option<String>,
    pub farm_name: Option<String>,
    pub location: Option<String>,
    pub house_code: String,
    #[serde(with = "rfc3339_time")]
    pub alarm_time: OffsetDateTime,
    pub day_age: Option<u32>,
    pub target_name: String,
    pub alarm_item: String,
    pub alarm_type: String,
    pub content: String,
}

impl From<AlarmInitRespItem> for Alarm {
    fn from(value: AlarmInitRespItem) -> Self {
        Self {
            house_code: value.house_code,
            tenant_id: Default::default(),
            farm_id: value.farm_id,
            target_name: value.target_name,
            alarm_item: value.alarm_item,
            content: value.content,
            timestamp: value.alarm_time,
            received_time: Some(value.alarm_time),
            alarm_type: value.alarm_type,
            is_confirmed: false,
            is_test: false,
            is_alarm: true,
            day_age: value.day_age,
            test_plan_time: None,
            test_time: None,
        }
    }
}

pub enum AlarmStatus {
    Playable,
    Canceled,
    Paused,
}

#[derive(Clone)]
pub struct House {
    /// 舍号/鸡舍名称
    pub name: String,
    /// 鸡舍码
    pub code: String,
    /// 是否启用
    pub enabled: bool,
    /// 是否空舍状态
    pub is_empty_mode: bool,
}

impl From<sys_house::Model> for House {
    fn from(value: sys_house::Model) -> Self {
        Self {
            name: value.name,
            code: value.house_code,
            enabled: value.enabled,
            is_empty_mode: value.is_empty,
        }
    }
}

#[derive(Default, Clone)]
pub struct BoxConfig {
    pub enabled: bool,
    pub volume: u32,
}

#[derive(Debug, Default, Clone)]
pub struct PostConfig {
    pub device_ids: Vec<u32>,
    pub speed: u8,
}

#[derive(Default, Clone)]
pub struct AlarmService {
    // 测试报警触发 crontab 表达方式
    pub crontab: Option<String>,
    // 报警播放延时
    pub play_delay_secs: u64,
    // 报警暂停
    pub is_alarm_paused: bool,
    // 报警快照集合，已取消报警直接移除
    pub alarm_set: HashMap<String, Alarm>,
    // 为匹配的取消报警集合
    pub unmapped_cancel_set: HashMap<String, Alarm>,
    /// 鸡舍状态
    pub house_set: HashMap<String, House>,
    /// 鸡场语言
    pub language: Option<String>,
    /// 默认语言
    pub default_language: String,
    /// 测试报警持续时长
    pub test_play_duration: u64,
    /// 国际化本地配置
    pub localization_set: HashMap<String, Localization>,
    /// 音箱配置
    pub soundbox: BoxConfig,
    /// 音柱配置
    pub soundposts: PostConfig,
    /// 循环播放间隔
    pub play_interval_secs: u64,
    /// 报警初始化接口地址
    pub alarms_init_url: String,
    /// Database conntection config
    pub dbconfig: DbConfig,
    /// 数据库连接
    pub db: Option<DatabaseConnection>,
}

impl AlarmService {
    pub fn new(
        play_delay_secs: u64,
        default_language: String,
        test_play_duration: u64,
        play_interval_secs: u64,
        alarms_init_url: String,
        dbconfig: DbConfig,
    ) -> Self {
        Self {
            play_delay_secs,
            is_alarm_paused: false,
            alarm_set: HashMap::new(),
            unmapped_cancel_set: HashMap::new(),
            house_set: HashMap::new(),
            default_language,
            test_play_duration,
            localization_set: HashMap::new(),
            soundbox: BoxConfig {
                enabled: true,
                volume: 100,
            },
            play_interval_secs,
            alarms_init_url,
            dbconfig,
            ..Default::default()
        }
    }

    pub async fn init(&mut self, localization_path: String) -> anyhow::Result<()> {
        self.init_localization_set(localization_path);
        self.connect_db().await?;

        if let Some(db) = self.db.clone() {
            self.init_house_set(&db).await?;
            if let Some(farm) = farm_config_info::find_one(&db).await? {
                self.is_alarm_paused = match farm.sound_column_pause {
                    Some(pause) => pause == 1,
                    None => false,
                };
                self.language = farm.alarm_content_lang;
                self.soundbox = BoxConfig {
                    enabled: match farm.speaker_state {
                        Some(state) => state == 1,
                        None => false,
                    },
                    volume: match farm.local_volume {
                        Some(volume) => volume as u32,
                        None => 50,
                    },
                }
            }

            self.soundposts = PostConfig {
                device_ids: Vec::new(),
                speed: 50,
            };

            let sc_list = sound_column_config::find_all(&db).await?;
            for sc in sc_list {
                if !sc.enabled {
                    continue;
                }
                self.soundposts.device_ids.push(sc.device_id as u32);
                self.soundposts.speed = sc.speed as u8;
            }

            let tac = test_alarm_config::find_one(&db).await?;
            if let Some(tac) = tac {
                if let Some(duration) = tac.duration {
                    self.test_play_duration = duration as u64;
                }
                self.crontab = tac.cron;
            }
        }

        Ok(())
    }

    async fn init_house_set(&mut self, db: &DatabaseConnection) -> anyhow::Result<()> {
        let models = sys_house::find_all(db).await?;
        for model in models {
            let house: House = model.into();
            let code = house.clone().code;
            self.house_set.insert(code, house);
        }

        Ok(())
    }

    async fn connect_db(&mut self) -> anyhow::Result<()> {
        let mut opt = ConnectOptions::new(self.dbconfig.connection());

        opt.sqlx_logging(true);
        opt.set_schema_search_path("public");
        if let Some(max_conns) = self.dbconfig.max_conns() {
            opt.max_connections(max_conns);
        }

        if let Some(min_conns) = self.dbconfig.min_conns() {
            opt.min_connections(min_conns);
        }

        if let Some(conn_timeout) = self.dbconfig.conn_timeout_millis() {
            opt.connect_timeout(Duration::from_millis(conn_timeout));
        }

        if let Some(idle_timeout) = self.dbconfig.idle_timeout_millis() {
            opt.idle_timeout(Duration::from_millis(idle_timeout));
        }

        if let Some(level) = self.dbconfig.logging_level() {
            opt.sqlx_logging_level(LevelFilter::from_str(level.as_str())?);
        }

        match Database::connect(opt).await {
            Ok(conn) => {
                self.db = Some(conn);
                Ok(())
            }
            Err(e) => {
                anyhow::bail!(
                    "Failed connecting to db: {}, err: {}",
                    self.dbconfig.connection(),
                    e
                );
            }
        }
    }

    fn init_localization_set(&mut self, localization_path: String) {
        if let Ok(entries) = fs::read_dir(localization_path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "json") {
                        match fs::read_to_string(&path) {
                            Ok(content) => match serde_json::from_str::<Localization>(&content) {
                                Ok(localization) => {
                                    self.localization_set
                                        .insert(localization.culture.clone(), localization);
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to deserialize file: {}: {e}, skipped.",
                                        path.display()
                                    );
                                }
                            },
                            Err(e) => {
                                error!("Failed to read file: {}: {e}, skipped.", path.display())
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_alarm_set_key(alarm: &Alarm) -> String {
        format!("{}_{}", alarm.house_code, alarm.target_name)
    }

    pub fn set_house_status(&mut self, house_code: String, enabled: bool, is_empty_mode: bool) {
        if let Some(house) = self.house_set.get_mut(&house_code) {
            house.enabled = enabled;
            house.is_empty_mode = is_empty_mode
        }
    }

    pub fn get_alarms(&self) -> Vec<Alarm> {
        self.alarm_set.values().cloned().collect()
    }

    pub fn set_alarm(&mut self, alarm: Alarm) -> bool {
        let key = Self::get_alarm_set_key(&alarm);
        match self.alarm_set.get(&key) {
            Some(last_alarm) => {
                if alarm.timestamp < last_alarm.timestamp {
                    warn!(
                        "Invalid alarm timestamp: {}, last_alarm timestamp: {}",
                        alarm.timestamp, last_alarm.timestamp
                    );
                    return false;
                }
                if !alarm.is_alarm {
                    // 消警，删除报警缓存
                    self.alarm_set.remove(&key);
                }
                return false;
            }
            None => {
                if alarm.is_alarm {
                    let _ = self.alarm_set.insert(key, alarm);
                    return true;
                }

                self.unmapped_cancel_set.insert(key, alarm);

                return false;
            }
        }
    }

    pub async fn init_alarm_set(&mut self) -> anyhow::Result<()> {
        let resp: AlarmsInitResp = reqwest::get(self.alarms_init_url.clone())
            .await
            .inspect_err(|e| error!("Failed for requesting the latest alarms: {e}"))?
            .json()
            .await
            .inspect_err(|e| error!("Failed for deserialize latest alarms response: {e}"))?;

        for item in resp.items {
            let alarm = item.into();
            let key = Self::get_alarm_set_key(&alarm);
            self.alarm_set.insert(key, alarm);
        }

        for k in self.unmapped_cancel_set.keys() {
            self.alarm_set.remove(k);
        }

        Ok(())
    }

    pub fn is_ongoing_alarm_exist(&self) -> bool {
        !self.alarm_set.is_empty()
    }

    pub fn get_alarm_status(&self, alarm: &Alarm) -> AlarmStatus {
        let key = Self::get_alarm_set_key(&alarm);
        if !self.alarm_set.contains_key(&key) {
            // 不存在，说明报警已经被取消
            return AlarmStatus::Canceled;
        }

        // 报警暂停
        if self.is_alarm_paused {
            return AlarmStatus::Paused;
        }

        // 空舍
        let paused = match self.house_set.get(&alarm.house_code) {
            Some(house) => !house.is_empty_mode && house.enabled,
            None => false,
        };
        if self.is_alarm_paused || alarm.is_confirmed || paused {
            return AlarmStatus::Paused;
        }

        return AlarmStatus::Playable;
    }

    pub fn next_fire_time(&self) -> Option<OffsetDateTime> {
        match &self.crontab {
            Some(crontab) => match Schedule::from_str(crontab.as_str()) {
                Ok(schedule) => {
                    if let Some(dt) = schedule.upcoming(Utc).next() {
                        match OffsetDateTime::from_unix_timestamp(dt.timestamp()) {
                            Ok(t) => return Some(t),
                            Err(e) => {
                                error!("Datetime convert failed: {e}");
                                return None;
                            }
                        }
                    }
                    error!("Invalid crontab...");
                    return None;
                }
                Err(e) => {
                    error!("Crontab parse failed: {e}");
                    return None;
                }
            },
            None => {
                warn!("Crontab is empty...");
                return None;
            }
        }
    }

    pub fn set_crontab(&mut self, ct: String) {
        self.crontab = Some(ct);
    }

    pub fn get_play_delay(&self) -> time::Duration {
        time::Duration::seconds(self.play_delay_secs as i64)
    }

    pub fn set_play_delay(&mut self, play_delay_secs: u64) {
        self.play_delay_secs = play_delay_secs;
    }

    pub fn set_language(&mut self, language: String) {
        self.language = Some(language);
    }

    pub fn get_test_play_duration(&self) -> u64 {
        self.test_play_duration.clone()
    }

    pub fn set_test_play_duration(&mut self, duration: u64) {
        self.test_play_duration = duration
    }

    pub fn get_alarm_content(&self, alarm: &Alarm) -> anyhow::Result<String> {
        let house_name = match self.house_set.get(&alarm.house_code) {
            Some(house) => house.name.clone(),
            None => anyhow::bail!("House not exist with code: {}", alarm.house_code),
        };

        let status = match alarm.content.split(" ").last() {
            Some(content) => content,
            None => {
                anyhow::bail!("Valid alarm content is empty, origin: {}", alarm.content);
            }
        };

        let (alarm_item, status) = match self.language.clone() {
            Some(ln) => {
                if ln == self.default_language {
                    (alarm.alarm_item.clone(), status)
                } else {
                    match self.localization_set.get(&ln) {
                        Some(localization) => {
                            let alarm_item = match localization.texts.get(&alarm.alarm_item) {
                                Some(txt) => txt.clone(),
                                None => {
                                    error!(
                                        "Content:{} not matched in language configuration, use origin.",
                                        alarm.alarm_item
                                    );
                                    alarm.alarm_item.clone()
                                }
                            };

                            if status == "" {
                                (alarm_item, status)
                            } else {
                                let status = match localization.texts.get(status) {
                                    Some(txt) => txt,
                                    None => {
                                        error!(
                                            "Status:{} not matched in language configuration, use origin",
                                            status
                                        );
                                        status
                                    }
                                };

                                (alarm_item, status)
                            }
                        }
                        None => {
                            error!("Language: {} not supported, use origin.", ln);
                            (alarm.alarm_item.clone(), status)
                        }
                    }
                }
            }
            None => {
                error!("Language not setted, use origin content.");
                (alarm.alarm_item.clone(), status)
            }
        };

        Ok(format!("[{house_name}] {alarm_item} {status}"))
    }

    pub fn set_soundbox(&mut self, soundbox: BoxConfig) {
        self.soundbox = soundbox;
    }

    pub fn get_soundbox(&self) -> BoxConfig {
        self.soundbox.clone()
    }

    pub fn set_soundposts(&mut self, soundposts: PostConfig) {
        self.soundposts = soundposts;
    }

    pub fn get_soundposts(&self) -> PostConfig {
        self.soundposts.clone()
    }

    pub fn set_play_interval_secs(&mut self, play_interval_secs: u64) {
        self.play_interval_secs = play_interval_secs;
    }

    pub fn get_play_interval_secs(&self) -> u64 {
        self.play_delay_secs.clone()
    }

    pub async fn play_record(&mut self, alarm: &Alarm, result: PlayResult) {
        info!(
            "Add play record, id: {}, has_error: {}, alarm: {:?}",
            result.id, result.has_error, alarm
        );
    }

    pub async fn test_play_record(&mut self, alarm: &Alarm, result: PlayResult) {
        let uuid = uuid::Uuid::new_v4();
        let now = match OffsetDateTime::now_local() {
            Ok(local) => local,
            Err(e) => {
                error!("Failed for getting local time: {e}");
                OffsetDateTime::now_utc()
            }
        };

        let ct = PrimitiveDateTime::new(now.date(), now.time());

        let plan_time = match alarm.test_plan_time {
            Some(t) => t,
            None => ct.clone(),
        };
        let test_time = match alarm.test_time {
            Some(t) => t,
            None => ct.clone(),
        };
        let model = test_alarm_play_record::Model {
            id: uuid,
            plan_time,
            test_time,
            test_type: 1,
            notify_obj: None,
            media_file: Some(result.id),
            test_result: match result.result_type {
                PlayResultType::Normal | PlayResultType::Timeout => 1,
                PlayResultType::Canceled(ct) => match ct {
                    PlayCancelType::AlarmArrived => 4,
                    PlayCancelType::Terminated => 5,
                },
            },
            has_error: result.has_error,
            err_message: result.err_message,
            creation_time: ct,
        };

        if let Some(db) = self.db.clone() {
            if let Err(e) = test_alarm_play_record::insert(model, &db).await {
                error!("Failed for insertting test alarm: {e}");
            }
        } else {
            error!("Database is not connected!")
        }
    }
}

pub struct PlayResult {
    pub id: String,
    pub has_error: bool,
    pub err_message: Option<String>,
    pub result_type: PlayResultType,
}

#[derive(Default, Clone, Debug, Deserialize)]
pub struct Localization {
    pub culture: String,
    pub texts: HashMap<String, String>,
}

#[cfg(test)]
mod service_tests {
    use tracing::info;

    use crate::service::AlarmsInitResp;

    #[tokio::test]
    async fn test_desc() {
        let body = reqwest::get(
            "http://192.168.77.34/api/IB/alarm-info/current-alarm-info-page-list-with-no-auth",
        )
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

        let result: AlarmsInitResp = serde_json::from_str(body.as_str()).unwrap();
        info!("result: {:?}", result);
    }
}
