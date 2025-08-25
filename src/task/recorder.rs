#[derive(Clone)]
pub struct Recorder {}

impl Recorder {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn record(&self) {}

    pub async fn check_and_upload(&self) {}
}
