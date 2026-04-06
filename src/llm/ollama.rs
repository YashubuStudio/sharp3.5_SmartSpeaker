use anyhow::Result;
use log::{info, warn};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

use crate::config::LlmConfig;

/// 非ストリーミングリクエストのタイムアウト（秒）
/// 初回はモデルのVRAMロードが発生するため余裕を持たせる
const REQUEST_TIMEOUT_SECS: u64 = 300;
/// ストリーミング接続のタイムアウト（秒）— レスポンス開始までの待ち時間
/// 初回のモデルロードで時間がかかるGPU環境を考慮
const STREAM_CONNECT_TIMEOUT_SECS: u64 = 120;

/// LLM処理に関するエラー
#[derive(Debug, Error)]
pub enum LlmError {
    #[error("Ollama APIへの接続に失敗: {0}")]
    ConnectionError(String),
}

/// Ollamaストリーミングレスポンスの1チャンク
#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    pub response: String,
    pub done: bool,
}

/// Ollamaを使用したLLMエンジン
pub struct OllamaLlm {
    client: Client,
    endpoint: String,
    model: String,
    system_prompt: String,
}

impl OllamaLlm {
    /// 設定からOllamaLlmインスタンスを生成
    ///
    /// # Arguments
    /// * `config` - LLM設定
    ///
    /// # Returns
    /// 初期化されたOllamaLlmインスタンス
    pub fn new(config: &LlmConfig) -> Result<Self> {
        info!(
            "Ollama LLM初期化: endpoint={}, model={}",
            config.endpoint, config.model
        );

        // 非localhostエンドポイントへの警告
        warn_if_non_localhost(&config.endpoint, "Ollama");

        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(STREAM_CONNECT_TIMEOUT_SECS))
            .build()
            .map_err(|e| LlmError::ConnectionError(format!("HTTPクライアント構築失敗: {}", e)))?;

        Ok(Self {
            client,
            endpoint: config.endpoint.clone(),
            model: config.model.clone(),
            system_prompt: config.system_prompt.clone(),
        })
    }

    /// Ollamaサーバーの接続確認
    pub fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.endpoint);

        match self.client.get(&url).send() {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// エンドポイントURLを取得
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// モデル名を取得
    pub fn model(&self) -> &str {
        &self.model
    }

    /// システムプロンプトを取得
    pub fn system_prompt(&self) -> &str {
        &self.system_prompt
    }

    /// 単発リクエストで応答テキストを生成
    pub fn generate(&self, prompt: &str) -> Result<String> {
        #[derive(Serialize)]
        struct GenerateRequest<'a> {
            model: &'a str,
            prompt: &'a str,
            system: &'a str,
            stream: bool,
        }

        #[derive(Deserialize)]
        struct GenerateResponse {
            response: String,
        }

        let request = GenerateRequest {
            model: &self.model,
            prompt,
            system: &self.system_prompt,
            stream: false,
        };

        let url = format!("{}/api/generate", self.endpoint);
        let response = self.client.post(&url).json(&request).send()?;
        if !response.status().is_success() {
            anyhow::bail!("Ollama APIエラー: ステータスコード {}", response.status());
        }
        let generated: GenerateResponse = response.json()?;
        Ok(generated.response.trim().to_string())
    }
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
