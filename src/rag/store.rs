use anyhow::{Context, Result};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// ベクトルストアに保存するドキュメント
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub doc_type: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    /// 重要度 (1-5)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importance: Option<u8>,
    /// 感情 (joy|sadness|anger|surprise|fear|neutral)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sentiment: Option<String>,
}

/// 検索結果
pub struct SearchResult<'a> {
    pub document: &'a Document,
    pub score: f32,
}

/// JSONファイル永続化 + インメモリベクトルストア
pub struct VectorStore {
    documents: Vec<Document>,
    file_path: PathBuf,
    /// 追記用ファイルハンドル（初回書き込み時にオープン、以降保持）
    writer: Option<File>,
}

impl VectorStore {
    /// store.jsonl を読み込んでメモリにロード
    pub fn load(file_path: &Path) -> Result<Self> {
        let mut documents = Vec::new();

        if file_path.exists() {
            let file = fs::File::open(file_path)
                .with_context(|| format!("store.jsonlの読み込みに失敗: {}", file_path.display()))?;
            let reader = BufReader::new(file);

            for (i, line) in reader.lines().enumerate() {
                let line = line.with_context(|| format!("store.jsonl {}行目の読み込みに失敗", i + 1))?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<Document>(trimmed) {
                    Ok(doc) => documents.push(doc),
                    Err(e) => {
                        log::warn!("store.jsonl {}行目のパースに失敗（スキップ）: {}", i + 1, e);
                    }
                }
            }

            info!("VectorStore: {}件のドキュメントをロード", documents.len());
        } else {
            info!("VectorStore: 新規作成 ({})", file_path.display());
        }

        Ok(Self {
            documents,
            file_path: file_path.to_path_buf(),
            writer: None,
        })
    }

    /// 追記用ファイルハンドルを取得（初回呼び出し時にオープン）
    fn get_writer(&mut self) -> Result<&mut File> {
        if self.writer.is_none() {
            if let Some(parent) = self.file_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("データディレクトリの作成に失敗: {}", parent.display()))?;
            }

            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.file_path)
                .with_context(|| format!("store.jsonlのオープンに失敗: {}", self.file_path.display()))?;

            self.writer = Some(file);
        }
        Ok(self.writer.as_mut().unwrap())
    }

    /// ドキュメントを追加（メモリ + ファイル追記）
    pub fn add(&mut self, doc: Document) -> Result<()> {
        let json = serde_json::to_string(&doc)
            .with_context(|| "ドキュメントのシリアライズに失敗")?;

        let writer = self.get_writer()
            .with_context(|| "store.jsonlへの書き込み準備に失敗")?;
        writeln!(writer, "{}", json)
            .with_context(|| "store.jsonlへの書き込みに失敗")?;
        writer.flush()
            .with_context(|| "store.jsonlのフラッシュに失敗")?;

        debug!("ドキュメント追加: id={}, type={}", doc.id, doc.doc_type);

        // メモリに追加
        self.documents.push(doc);

        Ok(())
    }

    /// cosine similarity で top-k 検索
    pub fn search(&self, query_embedding: &[f32], top_k: usize, threshold: f32) -> Vec<SearchResult<'_>> {
        let mut results: Vec<SearchResult> = self
            .documents
            .iter()
            .filter_map(|doc| {
                let score = cosine_similarity(query_embedding, &doc.embedding);
                if score >= threshold {
                    Some(SearchResult {
                        document: doc,
                        score,
                    })
                } else {
                    None
                }
            })
            .collect();

        // スコア降順でソート
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

        results.truncate(top_k);

        debug!(
            "検索結果: {}件 (閾値: {}, top_k: {})",
            results.len(),
            threshold,
            top_k
        );

        results
    }

    /// 指定source_fileのドキュメントが存在するか確認
    pub fn has_source(&self, source_file: &str) -> bool {
        self.documents
            .iter()
            .any(|doc| doc.source_file.as_deref() == Some(source_file))
    }
}

/// cosine similarity 計算
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;

    for (ai, bi) in a.iter().zip(b.iter()) {
        dot += ai * bi;
        norm_a += ai * ai;
        norm_b += bi * bi;
    }

    let denominator = norm_a.sqrt() * norm_b.sqrt();
    if denominator == 0.0 {
        return 0.0;
    }

    dot / denominator
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert!((score - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert!(score.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let score = cosine_similarity(&a, &b);
        assert!((score - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_different_lengths() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }
}
