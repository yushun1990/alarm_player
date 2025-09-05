mod soundpost;
pub use soundpost::{PlayContent, Soundpost, SpeechLoop};

mod soundbox;
pub use soundbox::{Buffer, Soundbox};

/// 播放取消类型
#[derive(Debug, Clone)]
pub enum PlayCancelType {
    AlarmArrived,
    Terminated,
}

/// 播放结果类型
#[derive(Debug, Clone)]
pub enum PlayResultType {
    Normal,
    Timeout,
    Canceled(PlayCancelType),
}
