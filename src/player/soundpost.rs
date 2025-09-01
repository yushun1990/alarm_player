use std::time::Duration;

use reqwest::{
    Client, StatusCode,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

pub enum PlayContent {
    Url(String),
    Tts(String),
}

#[derive(Clone, Serialize)]
pub struct SpeechRequest {
    pub device_ids: Vec<u32>,
    pub url: Option<String>,
    pub text: Option<String>,
    pub speech: Option<u8>,
    pub volume: u8,
    #[serde(rename = "loop")]
    pub speech_loop: SpeechLoop,
}

#[derive(Clone, Serialize)]
pub struct SpeechLoop {
    pub duration: u64,
    pub times: u32,
    pub gap: u64,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct CancelResp {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Vec<CancelRespData>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct CancelRespData {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub id: u32,
    #[serde(default)]
    pub body: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct CancelResult {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct StatusResp {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Vec<StatusRespData>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct StatusRespData {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub id: u32,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct StatusResult {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Option<StatusResultData>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct StatusResultData {
    #[serde(default)]
    pub speech: bool,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct SpeechResp {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Vec<SpeechResult>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct SpeechResult {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub id: u32,
    #[serde(default)]
    pub body: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct SpeechResultData {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
}

#[derive(Clone)]
pub struct Soundpost {
    api_host: String,
    client: Client,
}

impl Soundpost {
    pub fn new(api_host: String, api_login_token: String) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(format!("Bearer {api_login_token}").as_str()).unwrap(),
        );
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap();

        Self { api_host, client }
    }

    // 取消播放，仅记录取消结果，不做取消结果判定
    pub async fn cancel(&self, device_ids: &Vec<u32>) {
        let result: CancelResp = match self
            .client
            .delete(format!(
                "http://{}/v1/speech?device_ids={}",
                self.api_host,
                Self::encode_device_ids(device_ids)
            ))
            .send()
            .await
        {
            Ok(res) => match res.json().await {
                Ok(res) => res,
                Err(e) => {
                    error!("Cancel play failed: {e}");
                    return;
                }
            },
            Err(e) => {
                error!("Soundpost cancel play failed: {e}");
                return;
            }
        };

        debug!("Cancel result: {:?}", result);
        if result.code != StatusCode::OK {
            error!("Cancel failed with message: {}", result.message);
            return;
        }

        for data in result.data {
            if data.code != StatusCode::OK {
                error!(
                    "Cancel failed for device: {}, error message: {}",
                    data.id, data.message
                );
                continue;
            }

            match serde_json::from_str::<CancelResult>(&data.body) {
                Ok(result) => {
                    if result.code != StatusCode::OK {
                        error!(
                            "Cancel failed for device:{}, error message: {}",
                            data.id, result.message
                        );
                        continue;
                    }
                    info!(
                        "Cancel successed for device:{} - {}",
                        data.id, result.message
                    );
                }
                Err(e) => error!("Cancel result deserialize failed: {e}"),
            }
        }
    }

    // 是否播放完成
    // 任意错误都视为未播放完成，使用者需要自行协调超时机制
    async fn is_play_finished(&self, device_ids: &Vec<u32>) -> bool {
        let result: StatusResp = match self
            .client
            .get(format!(
                "http://{}/v1/play_status?device_ids={}",
                self.api_host,
                Self::encode_device_ids(device_ids)
            ))
            .send()
            .await
        {
            Ok(res) => match res.json().await {
                Ok(res) => res,
                Err(e) => {
                    error!("Status resp deserialize failed: {e}");
                    return false;
                }
            },
            Err(e) => {
                error!("Failed for reading speecher status: {e}");
                return false;
            }
        };

        debug!("Soundpost play status: {:?}", result);

        if result.code != StatusCode::OK {
            error!(
                "Failed for reading speecher status with message: {}",
                result.message
            );
            return false;
        }

        for pid in result.data {
            if pid.code != StatusCode::OK {
                error!(
                    "Reading speecker status failed for device id:{}, with message: {}",
                    pid.id, pid.message
                );
                // 有一个未读出即判断为未完成
                return false;
            }

            match serde_json::from_str::<StatusResult>(pid.body.as_str()) {
                Ok(status) => {
                    if status.code != StatusCode::OK {
                        error!("Status result failed with message: {}", status.message);
                        return false;
                    }
                    if let Some(s) = status.data {
                        if s.speech {
                            // 有一个处于speech状态即视为未完成
                            return false;
                        }
                    }
                }
                Err(e) => {
                    error!(
                        "Status body deserialize failed for device id: {}, err: {}",
                        pid.id, e
                    );
                    return false;
                }
            }
        }

        info!("All speechers fininshed playing.");

        return true;
    }

    #[allow(unreachable_code)]
    pub async fn play(
        &self,
        device_ids: Vec<u32>,
        media: PlayContent,
        speed: Option<u8>,
        speech_loop: SpeechLoop,
    ) -> anyhow::Result<()> {
        // 先取消所有播放
        self.cancel(&device_ids).await;

        let request =
            Self::build_speech_request(device_ids.clone(), media, speed, speech_loop.clone());
        let resp: SpeechResp = self
            .client
            .post(format!("http://{}/v1/speech", self.api_host))
            .json(&request)
            .send()
            .await
            .inspect_err(|e| error!("Speech request failed: {e}"))?
            .json()
            .await
            .inspect_err(|e| error!("Speech result deserilize failed:{e}"))?;

        if resp.code != StatusCode::OK {
            return anyhow::bail!("Speech request failed with message: {}", resp.message);
        }

        for result in resp.data {
            if result.code != StatusCode::OK {
                return anyhow::bail!(
                    "Speecher play failed, device_id: {}, error message: {}",
                    result.id,
                    result.message,
                );
            }

            let result_data =
                serde_json::from_str::<SpeechResultData>(&result.body).inspect_err(|e| {
                    error!(
                        "Speecher play result deserialize failed, device_id: {}, error:{e}",
                        result.id
                    )
                })?;

            if result_data.code != StatusCode::OK {
                return anyhow::bail!(
                    "Speecher play failed, device_id: {}, with message: {}",
                    result.id,
                    result_data.message
                );
            }

            info!(
                "Speecher play success, device_id: {} - {}",
                result.id, result_data.message
            );
        }
        // 等待播放完成
        tokio::time::sleep(Duration::from_secs(speech_loop.duration)).await;

        // 循环检测是否播放完成
        match tokio::time::timeout(
            Duration::from_secs(1),
            self.wait_for_play_finished(&device_ids),
        )
        .await
        {
            Ok(_) => {}
            Err(_) => {
                warn!(
                    "Speecher not finished the playing in {} secs, try to cancel it ...",
                    speech_loop.duration
                );
                self.cancel(&device_ids).await;
            }
        }

        Ok(())
    }

    async fn wait_for_play_finished(&self, device_ids: &Vec<u32>) {
        while !self.is_play_finished(device_ids).await {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    fn build_speech_request(
        device_ids: Vec<u32>,
        media: PlayContent,
        speed: Option<u8>,
        speech_loop: SpeechLoop,
    ) -> SpeechRequest {
        let (url, text) = match media {
            PlayContent::Tts(tts) => (None, Some(tts)),
            PlayContent::Url(url) => (Some(url), None),
        };

        SpeechRequest {
            device_ids,
            url,
            text,
            speech: speed,
            volume: 100,
            speech_loop,
        }
    }

    fn encode_device_ids(device_ids: &Vec<u32>) -> String {
        device_ids
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(",")
    }
}

#[cfg(test)]
mod soundpost_tests {
    use crate::player::{PlayContent, Soundpost, SpeechLoop};
    use std::time::Duration;

    #[tokio::test]
    async fn test_play() {
        let player = Soundpost::new(
            "192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
        );

        let url =
            String::from("http://192.168.77.14:8080/music/246610693611b3e86da7971c4e5365b0.mp3");
        let _ = player
            .play(
                vec![1, 2],
                PlayContent::Url(url),
                None,
                SpeechLoop {
                    duration: 60,
                    times: 1,
                    gap: 2,
                },
            )
            .await;
    }

    #[tokio::test]
    async fn test_is_play_finished() {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let player = Soundpost::new(
            "192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
        );

        assert_eq!(player.is_play_finished(&vec![1, 2]).await, false);
    }

    #[tokio::test]
    async fn test_cancel() {
        let player = Soundpost::new(
            "192.168.77.14:8080".into(),
            "YWRtaW46YWRtaW5fYXBpX2tleQ==".into(),
        );
        player.cancel(&vec![1, 2]).await;
    }
}
