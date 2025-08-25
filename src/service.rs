use chrono::Utc;
use cron::Schedule;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::str::FromStr;
use time::OffsetDateTime;
use tracing::{error, warn};

use crate::model::Alarm;

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

pub enum PlayContent {
    Url(String),
    TTS(String),
}

pub trait AlarmService: Clone + Send + Sync {
    /// 设置鸡舍暂停/空舍状态
    fn set_house_status(&mut self, house_code: String, enabled: bool, is_empty_mode: bool);
    /// 获取当前报警列表
    fn get_alarms(&self) -> Vec<Alarm>;
    /// 添加报警
    fn set_alarm(&mut self, alarm: Alarm) -> bool;
    /// 是否存在正在进行的报警
    fn is_ongoing_alarm_exist(&self) -> bool;
    /// 获取报警状态
    fn get_alarm_status(&self, alarm: &Alarm) -> AlarmStatus;

    /// 测试报警定时任务下一次触发时间
    fn next_fire_time(&self) -> Option<OffsetDateTime>;
    /// 更新报警测试定时表达式
    fn set_crontab(&mut self, ct: String);
    /// 读取报警播放延时
    fn get_play_delay(&self) -> time::Duration;
    /// 更新报警播放延时
    fn set_play_delay(&mut self, play_delay_secs: u64);
    /// 更新语言
    fn set_language(&mut self, language: String);
    /// 更新报警测试持续时长
    fn set_test_play_duration(&mut self, duration: u64);
    /// 获取测试报警持续时长
    fn get_test_play_duration(&self) -> u64;
    /// 获取播放内容
    fn get_alarm_content(&self, alarm: &Alarm) -> PlayContent;
    /// 设置报警播放模式
    fn set_alarm_play_mode(&mut self, alarm_play_mode: String);
    /// 设置音柱列表
    fn set_soundposts(&mut self, soundposts: Vec<u32>);
    /// 读取音柱列表
    fn get_soundposts(&self) -> Vec<u32>;
}

#[derive(Clone)]
pub struct DefaultAlarmServiceImpl {
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
    /// 报警播放模式 music/tts
    pub alarm_play_mode: String,
    /// 报警音乐地址
    pub alarm_media_url: String,
    /// 测试报警地址
    pub test_media_url: String,
    /// 测试报警持续时长
    pub test_play_duration: u64,
    /// 国际化本地配置
    pub localization_set: HashMap<String, Localization>,
    /// 音柱列表
    pub soundposts: Vec<u32>,
}

impl DefaultAlarmServiceImpl {
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
}

impl Default for DefaultAlarmServiceImpl {
    fn default() -> Self {
        Self {
            crontab: Default::default(),
            play_delay_secs: 20,
            is_alarm_paused: false,
            alarm_set: HashMap::new(),
            house_set: HashMap::new(),
            language: Default::default(),
            alarm_play_mode: "tts".into(),
            alarm_media_url: "http://host.docker.internal:80/NewAlarm.wav".into(),
            test_media_url: "http://host.docker.internal:80/TestAlarm.wav".into(),
            test_play_duration: 30,
            localization_set: HashMap::new(),
            soundposts: Vec::new(),
        }
    }
}

impl AlarmService for DefaultAlarmServiceImpl {
    fn set_house_status(&mut self, house_code: String, enabled: bool, is_empty_mode: bool) {
        if let Some(house) = self.house_set.get_mut(&house_code) {
            house.enabled = enabled;
            house.is_empty_mode = is_empty_mode
        }
    }

    fn get_alarms(&self) -> Vec<Alarm> {
        self.alarm_set.values().cloned().collect()
    }

    fn set_alarm(&mut self, alarm: Alarm) -> bool {
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

    fn is_ongoing_alarm_exist(&self) -> bool {
        !self.alarm_set.is_empty()
    }

    fn get_alarm_status(&self, alarm: &Alarm) -> AlarmStatus {
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

    fn next_fire_time(&self) -> Option<OffsetDateTime> {
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

    fn set_crontab(&mut self, ct: String) {
        self.crontab = Some(ct);
    }

    fn get_play_delay(&self) -> time::Duration {
        time::Duration::seconds(self.play_delay_secs as i64)
    }

    fn set_play_delay(&mut self, play_delay_secs: u64) {
        self.play_delay_secs = play_delay_secs;
    }

    fn set_language(&mut self, language: String) {
        self.language = Some(language);
    }

    fn get_test_play_duration(&self) -> u64 {
        self.test_play_duration.clone()
    }

    fn set_test_play_duration(&mut self, duration: u64) {
        self.test_play_duration = duration
    }

    fn get_alarm_content(&self, alarm: &Alarm) -> PlayContent {
        if self.alarm_play_mode == "music" {
            return PlayContent::Url(self.alarm_media_url.clone());
        }

        let house_name = match self.house_set.get(&alarm.house_code) {
            Some(house) => house.name.clone(),
            None => "".into(),
        };

        let status = match alarm.content.split(" ").last() {
            Some(content) => content,
            None => "",
        };

        let (alarm_item, status) = match self.language.clone() {
            Some(ln) => match self.localization_set.get(&ln) {
                Some(localization) => {
                    let alarm_item = match localization.texts.get(&alarm.alarm_item) {
                        Some(txt) => txt.clone(),
                        None => alarm.alarm_item.clone(),
                    };

                    if status == "" {
                        (alarm_item, status)
                    } else {
                        let status = match localization.texts.get(status) {
                            Some(txt) => txt,
                            None => "",
                        };

                        (alarm_item, status)
                    }
                }
                None => (alarm.alarm_item.clone(), status),
            },
            None => (alarm.alarm_item.clone(), status),
        };

        PlayContent::TTS(format!("[{house_name}] {alarm_item} {status}"))
    }

    fn set_alarm_play_mode(&mut self, alarm_play_mode: String) {
        for s in vec!["music", "tts"] {
            if alarm_play_mode == s {
                self.alarm_play_mode = alarm_play_mode;
                return;
            }
        }
    }

    fn set_soundposts(&mut self, mut soundposts: Vec<u32>) {
        self.soundposts.clear();
        self.soundposts.append(&mut soundposts);
    }

    fn get_soundposts(&self) -> Vec<u32> {
        self.soundposts.clone()
    }
}

#[derive(Default, Clone, Debug, Deserialize)]
pub struct Localization {
    pub culture: String,
    pub texts: HashMap<String, String>,
}
