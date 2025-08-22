use std::sync::Arc;

use alarm_player::{app, service::DefaultAlarmServiceImpl};
use tokio::sync::RwLock;

#[tokio::main]
async fn main() {
    app::run::<DefaultAlarmServiceImpl>(Arc::new(RwLock::new(DefaultAlarmServiceImpl::new(
        None,
        20,
        true,
        Vec::new(),
        Vec::new(),
    ))))
    .await;
}
