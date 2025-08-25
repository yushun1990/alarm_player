use std::sync::Arc;

use alarm_player::{app, config::Args, service::DefaultAlarmServiceImpl};
use clap::Parser;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = alarm_player::config::Config::new(args.config.as_str()).unwrap();
    app::run::<DefaultAlarmServiceImpl>(
        Arc::new(RwLock::new(DefaultAlarmServiceImpl::default())),
        config,
    )
    .await;
}
