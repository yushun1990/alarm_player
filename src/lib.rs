pub mod app;
pub mod config;
pub mod handler;
pub mod model;
pub mod mqtt_client;
pub mod player;
pub mod service;
pub mod task;

mod recorder;
use std::sync::Arc;

use mimalloc::MiMalloc;
pub use recorder::Recorder;

mod util;
use service::AlarmService;
use tokio::sync::RwLock;
pub use util::rfc3339_time;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

// {"code": 0, "message": "Success", "data": {"planTime": "", "testTime": "", "result": 3}}
pub const TOPIC_RESULT_CRONTAB: &str = "ap/test_alarm/crontab/result";
// {"duration": 120, "crontab": "0 12 * * * * *", "playNow": false}
pub const TOPIC_CRONTAB: &str = "ap/test_alarm/crontab";
pub const TOPIC_ALARM: &str = "$share/ap/+/+/alarm";
pub const TOPIC_REPUB_ALARM: &str = "$share/ap/+/+/repub_alarms";
// {"device_id": 1, "status": "online"}
pub const TOPIC_SOUNDPOST_STATUS: &str = "ap/soundpost/status";
// {"pause": true, "lang": "zh_Hans", "enableBox": true}
pub const TOPIC_FARM_CONFIG: &str = "ap/alarm/farm_config";
// {"deviceIds": [1,  2], "speed": 50}
pub const TOPIC_SOUND_POST: &str = "ap/device/sound_posts";
// [{"name": "9200", "code": "h42k3433", "enabled": true, "isEmptyMode": false}, ..]
pub const TOPIC_HOUSE_SET: &str = "ap/alarm/houses";
// [{"houseCode": "d2123sd333", "targetName": "高温报警", "isConfirmed": true}]
pub const TOPIC_ALARM_CONFIRM: &str = "ap/alarm/confirm";

type Service = Arc<RwLock<AlarmService>>;
