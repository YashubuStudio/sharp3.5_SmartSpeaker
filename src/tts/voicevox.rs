use anyhow::Result;
use log::{info, warn};
use reqwest::blocking::Client;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;

use crate::config::TtsConfig;

/// TTS APIリクエストのタイムアウト（秒）
const REQUEST_TIMEOUT_SECS: u64 = 60;
/// TTS API接続タイムアウト（秒）
const CONNECT_TIMEOUT_SECS: u64 = 10;
/// 音声データの最大サイズ（50MB — 長文合成時の上限）
const MAX_AUDIO_SIZE: usize = 50 * 1024 * 1024;

/// TTS処理に関するエラー
#[derive(Debug, Error)]
pub enum TtsError {
    #[error("VOICEVOX APIへの接続に失敗: {0}")]
    ConnectionError(String),

    #[error("音声クエリの作成に失敗: {0}")]
    AudioQueryError(String),

    #[error("音声合成に失敗: {0}")]
    SynthesisError(String),
}

/// VOICEVOXを使用した音声合成エンジン
pub struct VoicevoxTts {
    client: Client,
    endpoint: String,
    speaker_id: i32,
    speed: f32,
}

impl VoicevoxTts {
    /// 設定からVoicevoxTtsインスタンスを生成
    ///
    /// # Arguments
    /// * `config` - TTS設定
    ///
    /// # Returns
    /// 初期化されたVoicevoxTtsインスタンス
    pub fn new(config: &TtsConfig) -> Result<Self> {
        info!(
            "VOICEVOX TTS初期化: endpoint={}, speaker_id={}",
            config.endpoint, config.speaker_id
        );

        // 非localhostエンドポイントへの警告
        warn_if_non_localhost(&config.endpoint, "VOICEVOX");

        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .build()
            .map_err(|e| TtsError::ConnectionError(format!("HTTPクライアント構築失敗: {}", e)))?;

        Ok(Self {
            client,
            endpoint: config.endpoint.clone(),
            speaker_id: config.speaker_id,
            speed: config.speed,
        })
    }

    /// エンドポイントURLを取得
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// 話者IDを取得
    pub fn speaker_id(&self) -> i32 {
        self.speaker_id
    }

    /// 話速を取得
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// VOICEVOXサーバーの接続確認
    pub fn health_check(&self) -> Result<bool> {
        let url = format!("{}/version", self.endpoint);

        match self.client.get(&url).send() {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// テキストを音声データ（WAV）に変換
    pub fn synthesize(&self, text: &str) -> Result<Vec<u8>> {
        synthesize_with_client(
            &self.client,
            &self.endpoint,
            self.speaker_id,
            self.speed,
            text,
        )
    }
}

/// スタンドアロンTTS合成関数（Send制約回避用）
///
/// `VoicevoxTts` インスタンスを持てないスレッドから呼び出すために、
/// 必要なパラメータとHTTPクライアントを直接受け取る。
pub fn synthesize_with_client(
    client: &Client,
    endpoint: &str,
    speaker_id: i32,
    speed: f32,
    text: &str,
) -> Result<Vec<u8>> {
    // 1. audio_query作成
    let query_url = format!(
        "{}/audio_query?text={}&speaker={}",
        endpoint,
        urlencoding::encode(text),
        speaker_id
    );

    let response = client
        .post(&query_url)
        .send()
        .map_err(|e| TtsError::ConnectionError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(
            TtsError::AudioQueryError(format!("ステータスコード: {}", response.status())).into(),
        );
    }

    let mut query: Value = response
        .json()
        .map_err(|e| TtsError::AudioQueryError(e.to_string()))?;

    // 話速を設定
    if let Some(obj) = query.as_object_mut() {
        obj.insert("speedScale".to_string(), Value::from(speed));
    }

    // 2. synthesis実行
    let synth_url = format!("{}/synthesis?speaker={}", endpoint, speaker_id);

    let response = client
        .post(&synth_url)
        .header("Content-Type", "application/json")
        .json(&query)
        .send()
        .map_err(|e| TtsError::ConnectionError(e.to_string()))?;

    if !response.status().is_success() {
        return Err(
            TtsError::SynthesisError(format!("ステータスコード: {}", response.status())).into(),
        );
    }

    let audio = response
        .bytes()
        .map_err(|e| TtsError::SynthesisError(e.to_string()))?
        .to_vec();

    if audio.len() > MAX_AUDIO_SIZE {
        return Err(TtsError::SynthesisError(format!(
            "音声データが上限サイズを超過: {} bytes (上限: {} bytes)",
            audio.len(),
            MAX_AUDIO_SIZE
        ))
        .into());
    }

    Ok(audio)
}

/// 非localhostエンドポイントに対して警告を出力する
fn warn_if_non_localhost(endpoint: &str, service_name: &str) {
    if let Ok(url) = reqwest::Url::parse(endpoint) {
        let host = url.host_str().unwrap_or("");
        let is_local = host == "localhost"
            || host == "127.0.0.1"
            || host == "::1"
            || host == "[::1]"
            || host == "0.0.0.0";
        if !is_local {
            warn!(
                "セキュリティ警告: {} エンドポイント ({}) がlocalhostではありません。\
                 HTTP平文通信のため、中間者攻撃・盗聴のリスクがあります。\
                 HTTPS化を強く推奨します。",
                service_name, endpoint
            );
            if url.scheme() != "https" {
                warn!(
                    "セキュリティ警告: {} エンドポイントがHTTPSを使用していません。",
                    service_name
                );
            }
        }
    }
}
