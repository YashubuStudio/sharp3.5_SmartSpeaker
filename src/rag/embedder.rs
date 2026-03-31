use anyhow::{Context, Result};
use log::{debug, warn};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Embedding APIリクエストのタイムアウト（秒）
const REQUEST_TIMEOUT_SECS: u64 = 60;
/// Embedding API接続タイムアウト（秒）
const CONNECT_TIMEOUT_SECS: u64 = 10;
/// 許容する最大ベクトル次元数
const MAX_EMBEDDING_DIMENSIONS: usize = 8192;

/// Ollama Embedding API クライアント
pub struct OllamaEmbedder {
    client: Client,
    endpoint: String,
    model: String,
    expected_dimensions: Option<usize>,
}

#[derive(Debug, Serialize)]
struct EmbedRequest {
    model: String,
    input: String,
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

impl OllamaEmbedder {
    pub fn new(endpoint: &str, model: &str) -> Self {
        // 非localhostエンドポイントへの警告
        warn_if_non_localhost(endpoint, "Ollama Embedding");

        let client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            endpoint: endpoint.to_string(),
            model: model.to_string(),
            expected_dimensions: None,
        }
    }

    /// テキストをembeddingベクトルに変換
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embed", self.endpoint);

        let request = EmbedRequest {
            model: self.model.clone(),
            input: text.to_string(),
        };

        debug!("Embedding生成: \"{}...\"", &text.chars().take(50).collect::<String>());

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .with_context(|| "Ollama Embedding APIへの接続に失敗")?;

        if !response.status().is_success() {
            anyhow::bail!(
                "Ollama Embedding APIエラー: ステータスコード {}",
                response.status()
            );
        }

        let result: EmbedResponse = response
            .json()
            .with_context(|| "Embedding APIレスポンスのパースに失敗")?;

        let embedding = result
            .embeddings
            .into_iter()
            .next()
            .with_context(|| "Embedding APIから空のレスポンスが返されました")?;

        // ベクトル次元数の検証
        if embedding.len() > MAX_EMBEDDING_DIMENSIONS {
            anyhow::bail!(
                "Embeddingベクトルの次元数が上限を超過: {} (上限: {})",
                embedding.len(),
                MAX_EMBEDDING_DIMENSIONS
            );
        }

        // 初回取得時に次元数を記録し、以降は一貫性を検証
        match self.expected_dimensions {
            None => {
                debug!("Embedding次元数を記録: {}", embedding.len());
                self.expected_dimensions = Some(embedding.len());
            }
            Some(expected) if embedding.len() != expected => {
                anyhow::bail!(
                    "Embeddingベクトルの次元数が不一致: {} (期待値: {})",
                    embedding.len(),
                    expected
                );
            }
            _ => {}
        }

        Ok(embedding)
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
                 HTTP平文通信のため、中間者攻撃・盗聴のリスクがあります。",
                service_name, endpoint
            );
        }
    }
}
