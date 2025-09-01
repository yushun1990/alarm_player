use chrono::Utc;
use cron::Schedule;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::str::FromStr;
use time::OffsetDateTime;
use tracing::{error, info, warn};

use crate::{config::PlayMode, model::Alarm};

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
    /// 收否空舍状态
    pub is_empty_mode: bool,
}

#[derive(Default, Clone)]
pub struct BoxConfig {
    pub enabled: bool,
    pub volume: f32,
}

#[derive(Default, Clone)]
pub struct PostConfig {
    pub device_ids: Vec<u32>,
    pub speed: u8,
    pub duration: u64,
    pub play_mode: PlayMode,
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
    /// 音柱配置
    pub soundbox: BoxConfig,
    /// 音柱配置
    pub soundposts: PostConfig,
    /// 循环播放间隔
    pub play_interval_secs: u64,
}

impl AlarmService {
    pub fn new(
        play_delay_secs: u64,
        default_language: String,
        default_play_mode: PlayMode,
        test_play_duration: u64,
        play_interval_secs: u64,
    ) -> Self {
        Self {
            play_delay_secs,
            is_alarm_paused: false,
            alarm_set: HashMap::new(),
            house_set: HashMap::new(),
            default_language,
            test_play_duration,
            localization_set: HashMap::new(),
            soundbox: BoxConfig {
                enabled: true,
                volume: 0.5,
            },
            soundposts: PostConfig {
                device_ids: Vec::new(),
                speed: 50,
                duration: 60,
                play_mode: default_play_mode,
            },
            play_interval_secs,
            ..Default::default()
        }
    }
    pub fn init_localization_set(&mut self, localization_path: String) {
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
                let _ = self.alarm_set.insert(key, alarm);
                true
            }
        }
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
}

pub struct PlayResult {
    pub id: String,
    pub has_error: bool,
}

#[derive(Default, Clone, Debug, Deserialize)]
pub struct Localization {
    pub culture: String,
    pub texts: HashMap<String, String>,
}
