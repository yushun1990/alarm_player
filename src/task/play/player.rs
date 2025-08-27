use reqwest::{
    Client,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
pub struct SpeechRequest {
    pub device_ids: Vec<u32>,
    pub url: Option<String>,
    pub text: Option<String>,
    pub volume: u8,
    #[serde(rename = "loop")]
    pub play_loop: Option<SpeechLoop>,
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
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct CancelRespData {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub id: u32,
    pub body: Option<String>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct CancelResult {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct SpeecherInfo {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Vec<SpeecherInfoData>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct SpeecherInfoData {
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
pub struct SpeecherStatus {
    #[serde(default)]
    pub code: u16,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub data: Option<SpeecherStatusData>,
}

#[derive(Default, Debug, Clone, Deserialize)]
pub struct SpeecherStatusData {
    #[serde(default)]
    pub speech: bool,
}

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

    async fn cancel(&self, device_ids: &Vec<u32>) {
        let result = match self
            .client
            .delete(format!(
                "http://{}/v1/speech?device_ids={}",
                self.api_host,
                Self::get_device_id_params(device_ids)
            ))
            .send()
            .await
        {
            Ok(res) => match res.text().await {
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

        info!("Cancel result: {result}");
    }

    async fn encode_device_ids(device_ids: &Vec<u32>) -> String {
        device_ids
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>()
            .join(",")
    }
}
