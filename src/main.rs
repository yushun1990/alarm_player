use std::sync::Arc;

use alarm_player::{app, config::Args, service::AlarmService};
use clap::Parser;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = alarm_player::config::Config::new(args.config.as_str()).unwrap();
    let dbconfig = config.database.clone();
    let alarm_service = AlarmService::new(
        config.alarm.play_delay_secs(),
        config.alarm.default_langauge(),
        config.alarm.default_test_play_duration(),
        config.alarm.play_interval_secs(),
        config.alarm.init_url(),
        dbconfig,
    );

    app::run(Arc::new(RwLock::new(alarm_service)), config).await;
}
