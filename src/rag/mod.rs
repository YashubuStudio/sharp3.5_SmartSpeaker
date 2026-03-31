mod embedder;
mod store;

use anyhow::{Context, Result};
use log::{debug, info, warn};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::RagConfig;
use embedder::OllamaEmbedder;
use store::{Document, VectorStore};

/// 会話分析結果
#[derive(Debug, Deserialize)]
struct AnalysisResult {
    importance: u8,
    sentiment: String,
}

const VALID_SENTIMENTS: &[&str] = &["joy", "sadness", "anger", "surprise", "fear", "neutral"];

/// RAGエンジン — 検索拡張生成の公開API
pub struct RagEngine {
    // mut because embed() tracks dimension consistency
    embedder: OllamaEmbedder,
    store: VectorStore,
    data_dir: PathBuf,
    top_k: usize,
    similarity_threshold: f32,
    llm_endpoint: String,
    llm_model: String,
}

impl RagEngine {
    pub fn new(config: &RagConfig, ollama_endpoint: &str, llm_model: &str) -> Result<Self> {
        // 相対パスを絶対パスに変換（ネイティブライブラリがCWDを変更しても安全にする）
        let data_dir = if Path::new(&config.data_dir).is_relative() {
            std::env::current_dir()
                .with_context(|| "カレントディレクトリの取得に失敗")?
                .join(&config.data_dir)
        } else {
            PathBuf::from(&config.data_dir)
        };
        let store_path = data_dir.join("store.jsonl");
        let knowledge_dir = data_dir.join("knowledge");

        // データディレクトリとknowledgeディレクトリを作成
        fs::create_dir_all(&knowledge_dir)
            .with_context(|| format!("knowledgeディレクトリの作成に失敗: {}", knowledge_dir.display()))?;

        let embedder = OllamaEmbedder::new(ollama_endpoint, &config.embedding_model);
        let store = VectorStore::load(&store_path)?;

        Ok(Self {
            embedder,
            store,
            data_dir,
            top_k: config.top_k,
            similarity_threshold: config.similarity_threshold,
            llm_endpoint: ollama_endpoint.to_string(),
            llm_model: llm_model.to_string(),
        })
    }

    /// クエリに関連するドキュメントを検索してコンテキスト文字列を返す
    pub fn retrieve_context(&mut self, query: &str) -> Result<String> {
        let query_embedding = self.embedder.embed(query)?;

        let results = self.store.search(&query_embedding, self.top_k, self.similarity_threshold);

        if results.is_empty() {
            debug!("RAG: 関連ドキュメントなし");
            return Ok(String::new());
        }

        // ファクトのスコアをブーストしてソート
        let mut boosted: Vec<_> = results
            .iter()
            .map(|r| {
                let boosted_score = if r.document.doc_type == "fact" {
                    r.score * 1.2
                } else {
                    r.score
                };
                (r, boosted_score)
            })
            .collect();
        boosted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let context = boosted
            .iter()
            .enumerate()
            .map(|(i, (r, boosted_score))| {
                let mut meta = format!(
                    "[{}] (類似度: {:.2}, 種別: {}",
                    i + 1,
                    boosted_score,
                    r.document.doc_type,
                );
                if let Some(imp) = r.document.importance {
                    meta.push_str(&format!(", 重要度: {}", imp));
                }
                if let Some(ref sent) = r.document.sentiment {
                    meta.push_str(&format!(", 感情: {}", sent));
                }
                meta.push(')');
                format!("{}\n{}", meta, r.document.content)
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        debug!("RAG: {}件のコンテキストを取得", results.len());
        Ok(context)
    }

    /// ファクトを保存（embedding生成 + doc_type="fact" でストアに追記）
    pub fn save_fact(&mut self, fact: &str) -> Result<()> {
        let embedding = self.embedder.embed(fact)?;
        let timestamp = chrono::Local::now().to_rfc3339();
        let id = format!("fact_{}", chrono::Local::now().timestamp());

        let doc = Document {
            id,
            doc_type: "fact".to_string(),
            content: fact.to_string(),
            embedding,
            timestamp,
            source_file: None,
            importance: Some(3),
            sentiment: None,
        };

        self.store.add(doc)?;
        info!("ファクトを保存: {}", fact);
        Ok(())
    }

    /// 会話を保存（embedding生成+メタデータ分析+store追記）
    pub fn save_conversation(&mut self, user_query: &str, llm_response: &str) -> Result<()> {
        let content = format!("Q: {}\nA: {}", user_query, llm_response);
        let embedding = self.embedder.embed(&content)?;

        // メタデータ分析（失敗しても会話は保存する）
        let (importance, sentiment) = match self.analyze_conversation(user_query, llm_response) {
            Ok(analysis) => {
                debug!(
                    "会話分析結果: importance={}, sentiment={}",
                    analysis.importance, analysis.sentiment
                );
                (Some(analysis.importance), Some(analysis.sentiment))
            }
            Err(e) => {
                warn!("会話メタデータ分析に失敗（メタデータなしで保存）: {}", e);
                (None, None)
            }
        };

        let timestamp = chrono::Local::now().to_rfc3339();
        let id = format!("conv_{}", chrono::Local::now().timestamp());

        let doc = Document {
            id,
            doc_type: "conversation".to_string(),
            content,
            embedding,
            timestamp,
            source_file: None,
            importance,
            sentiment,
        };

        self.store.add(doc)?;
        debug!("会話を保存しました");

        Ok(())
    }

    /// LLMを使って会話の重要度と感情を分析
    fn analyze_conversation(&self, user_query: &str, llm_response: &str) -> Result<AnalysisResult> {
        let prompt = format!(
            r#"以下の会話の重要度と感情を分析してください。

会話:
ユーザー: {}
アシスタント: {}

重要度の基準:
1 = 挨拶・雑談
2 = 一般的な質問
3 = 実用的な情報要求
4 = 重要な判断・決定に関わる質問
5 = 緊急・安全に関わる質問

感情カテゴリ: joy, sadness, anger, surprise, fear, neutral

以下のJSON形式のみで回答してください（他のテキストは不要）:
{{"importance": 数値, "sentiment": "カテゴリ"}}"#,
            user_query, llm_response
        );

        let client = reqwest::blocking::ClientBuilder::new()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .with_context(|| "分析用HTTPクライアントの作成に失敗")?;

        let url = format!("{}/api/generate", self.llm_endpoint);
        let body = serde_json::json!({
            "model": self.llm_model,
            "prompt": prompt,
            "stream": false,
        });

        let resp = client
            .post(&url)
            .json(&body)
            .send()
            .with_context(|| "会話分析リクエストの送信に失敗")?;

        let resp_json: serde_json::Value = resp
            .json()
            .with_context(|| "会話分析レスポンスのパースに失敗")?;

        let response_text = resp_json["response"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let analysis = extract_json(&response_text)
            .with_context(|| format!("分析結果のJSON抽出に失敗: {}", response_text))?;

        // バリデーション: 重要度を1-5にクランプ
        let importance = analysis.importance.clamp(1, 5);

        // バリデーション: 感情カテゴリが有効か確認
        let sentiment = if VALID_SENTIMENTS.contains(&analysis.sentiment.as_str()) {
            analysis.sentiment
        } else {
            warn!(
                "不正な感情カテゴリ '{}' → neutralにフォールバック",
                analysis.sentiment
            );
            "neutral".to_string()
        };

        Ok(AnalysisResult {
            importance,
            sentiment,
        })
    }

    /// knowledge/ フォルダをスキャンして新規ファイルをインデックス
    pub fn index_knowledge(&mut self) -> Result<usize> {
        let knowledge_dir = self.data_dir.join("knowledge");

        if !knowledge_dir.exists() {
            return Ok(0);
        }

        let mut indexed_count = 0;

        let entries = fs::read_dir(&knowledge_dir)
            .with_context(|| format!("knowledgeディレクトリの読み込みに失敗: {}", knowledge_dir.display()))?;

        // 正規化されたknowledgeディレクトリパス（シンボリックリンク解決）
        let canonical_knowledge_dir = fs::canonicalize(&knowledge_dir)
            .with_context(|| format!("knowledgeディレクトリの正規化に失敗: {}", knowledge_dir.display()))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // 正規ファイルのみ許可（シンボリックリンクを除外）
            let metadata = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(e) => {
                    warn!("メタデータ取得失敗（スキップ）: {} - {}", path.display(), e);
                    continue;
                }
            };
            if metadata.file_type().is_symlink() {
                warn!(
                    "セキュリティ: シンボリックリンクを検出（スキップ）: {}",
                    path.display()
                );
                continue;
            }
            if !metadata.is_file() {
                continue;
            }

            // パストラバーサル防止: 正規化パスがknowledgeディレクトリ内にあるか検証
            match fs::canonicalize(&path) {
                Ok(canonical_path) => {
                    if !canonical_path.starts_with(&canonical_knowledge_dir) {
                        warn!(
                            "セキュリティ: パスがknowledgeディレクトリ外を指しています（スキップ）: {}",
                            path.display()
                        );
                        continue;
                    }
                }
                Err(e) => {
                    warn!("パス正規化失敗（スキップ）: {} - {}", path.display(), e);
                    continue;
                }
            }

            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // 既にインデックス済みのファイルはスキップ
            if self.store.has_source(&file_name) {
                debug!("スキップ（インデックス済み）: {}", file_name);
                continue;
            }

            let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

            let chunks = match extension {
                "json" => self.parse_json_knowledge(&path)?,
                "txt" => self.parse_text_knowledge(&path)?,
                _ => {
                    warn!("未対応のファイル形式（スキップ）: {}", file_name);
                    continue;
                }
            };

            for (i, chunk) in chunks.iter().enumerate() {
                if chunk.trim().is_empty() {
                    continue;
                }

                let embedding = self.embedder.embed(chunk)?;
                let timestamp = chrono::Local::now().to_rfc3339();
                let id = format!(
                    "know_{}_{}",
                    file_name.replace('.', "_"),
                    i
                );

                let doc = Document {
                    id,
                    doc_type: "knowledge".to_string(),
                    content: chunk.clone(),
                    embedding,
                    timestamp,
                    source_file: Some(file_name.clone()),
                    importance: None,
                    sentiment: None,
                };

                self.store.add(doc)?;
                indexed_count += 1;
            }

            info!("ナレッジインデックス完了: {} ({}チャンク)", file_name, chunks.len());
        }

        Ok(indexed_count)
    }

    /// JSONファイルをパースしてチャンクのリストを返す
    fn parse_json_knowledge(&self, path: &Path) -> Result<Vec<String>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("ファイル読み込み失敗: {}", path.display()))?;

        // 配列形式: [{"title": "...", "content": "..."}, ...]
        let items: Vec<serde_json::Value> = serde_json::from_str(&content)
            .with_context(|| format!("JSONパース失敗: {}", path.display()))?;

        let chunks = items
            .into_iter()
            .map(|item| {
                let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("");
                let body = item.get("content").and_then(|v| v.as_str()).unwrap_or("");
                if title.is_empty() {
                    body.to_string()
                } else {
                    format!("{}\n{}", title, body)
                }
            })
            .filter(|s| !s.trim().is_empty())
            .collect();

        Ok(chunks)
    }

    /// テキストファイルを空行区切りでチャンクに分割
    fn parse_text_knowledge(&self, path: &Path) -> Result<Vec<String>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("ファイル読み込み失敗: {}", path.display()))?;

        let chunks: Vec<String> = content
            .split("\n\n")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(chunks)
    }
}

/// LLMレスポンスからJSON部分を抽出してパース
fn extract_json(text: &str) -> Result<AnalysisResult> {
    let trimmed = text.trim();

    // 直接パースを試行
    if let Ok(result) = serde_json::from_str::<AnalysisResult>(trimmed) {
        return Ok(result);
    }

    // Markdownコードブロックから抽出を試行
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            let json_str = &trimmed[start..=end];
            if let Ok(result) = serde_json::from_str::<AnalysisResult>(json_str) {
                return Ok(result);
            }
        }
    }

    anyhow::bail!("JSONの抽出に失敗: {}", trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_raw() {
        let input = r#"{"importance": 3, "sentiment": "joy"}"#;
        let result = extract_json(input).unwrap();
        assert_eq!(result.importance, 3);
        assert_eq!(result.sentiment, "joy");
    }

    #[test]
    fn test_extract_json_with_markdown_fence() {
        let input = "```json\n{\"importance\": 2, \"sentiment\": \"neutral\"}\n```";
        let result = extract_json(input).unwrap();
        assert_eq!(result.importance, 2);
        assert_eq!(result.sentiment, "neutral");
    }

    #[test]
    fn test_extract_json_with_surrounding_text() {
        let input = "分析結果は以下です:\n{\"importance\": 4, \"sentiment\": \"surprise\"}\n以上です。";
        let result = extract_json(input).unwrap();
        assert_eq!(result.importance, 4);
        assert_eq!(result.sentiment, "surprise");
    }

    #[test]
    fn test_extract_json_invalid() {
        let input = "これはJSONではありません";
        assert!(extract_json(input).is_err());
    }

    #[test]
    fn test_importance_clamp() {
        // 0以下は1にクランプ、6以上は5にクランプ
        assert_eq!(0_u8.clamp(1, 5), 1);
        assert_eq!(6_u8.clamp(1, 5), 5);
        assert_eq!(3_u8.clamp(1, 5), 3);
    }

    #[test]
    fn test_valid_sentiments() {
        assert!(VALID_SENTIMENTS.contains(&"joy"));
        assert!(VALID_SENTIMENTS.contains(&"neutral"));
        assert!(!VALID_SENTIMENTS.contains(&"happy"));
        assert!(!VALID_SENTIMENTS.contains(&"unknown"));
    }

    #[test]
    fn test_sentiment_fallback() {
        let invalid = "happy";
        let result = if VALID_SENTIMENTS.contains(&invalid) {
            invalid.to_string()
        } else {
            "neutral".to_string()
        };
        assert_eq!(result, "neutral");
    }

    #[test]
    fn test_document_backward_compat() {
        // 旧形式（importance/sentimentなし）のJSONがデシリアライズできる
        let json = r#"{"id":"conv_1","doc_type":"conversation","content":"Q: hi\nA: hello","embedding":[0.1,0.2],"timestamp":"2024-01-01T00:00:00+09:00"}"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert!(doc.importance.is_none());
        assert!(doc.sentiment.is_none());
    }

    #[test]
    fn test_document_with_metadata() {
        let json = r#"{"id":"conv_2","doc_type":"conversation","content":"Q: test\nA: ok","embedding":[0.1],"timestamp":"2024-01-01T00:00:00+09:00","importance":3,"sentiment":"joy"}"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.importance, Some(3));
        assert_eq!(doc.sentiment.as_deref(), Some("joy"));
    }

    #[test]
    fn test_document_serialization_skips_none() {
        let doc = Document {
            id: "test".to_string(),
            doc_type: "conversation".to_string(),
            content: "test".to_string(),
            embedding: vec![0.1],
            timestamp: "2024-01-01".to_string(),
            source_file: None,
            importance: None,
            sentiment: None,
        };
        let json = serde_json::to_string(&doc).unwrap();
        assert!(!json.contains("importance"));
        assert!(!json.contains("sentiment"));
    }
}
