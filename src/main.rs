use std::sync::Arc;

use alarm_player::{app, config::Args, service::AlarmService};
use clap::Parser;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let config = alarm_player::config::Config::new(args.config.as_str()).unwrap();
    app::run(Arc::new(RwLock::new(AlarmService::default())), config).await;
}
