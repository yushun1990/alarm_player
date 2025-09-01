use std::sync::Arc;

use alarm_player::{app, config::Args, service::AlarmService};
use clap::Parser;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = alarm_player::config::Config::new(args.config.as_str()).unwrap();
    app::run(
        Arc::new(RwLock::new(AlarmService::new(
            config.alarm.play_delay_secs(),
            config.alarm.default_langauge(),
            config.soundpost.play_mode(),
            config.alarm.default_test_play_duration(),
            config.alarm.play_interval_secs(),
        ))),
        config,
    )
    .await;
}

#[cfg(test)]
mod main_tests {
    #[ctor::ctor]
    fn init() {
        tracing_subscriber::fmt().with_env_filter("info").init();
    }
}
