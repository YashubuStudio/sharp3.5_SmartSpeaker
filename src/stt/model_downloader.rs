use anyhow::{bail, Context, Result};
use log::info;
use std::fs;
use std::io::{self, BufRead, Read, Write};
use std::path::Path;

const HUGGINGFACE_BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

const PROGRESS_INTERVAL_BYTES: u64 = 10 * 1024 * 1024; // 10MB

/// Whisperモデルの情報
struct ModelInfo {
    file_name: &'static str,
    size_mb: u32,
    description: &'static str,
}

const MODELS: &[ModelInfo] = &[
    ModelInfo {
        file_name: "ggml-tiny.bin",
        size_mb: 75,
        description: "最小・最速（テスト用）",
    },
    ModelInfo {
        file_name: "ggml-base.bin",
        size_mb: 142,
        description: "軽量（低スペックPC向け）",
    },
    ModelInfo {
        file_name: "ggml-small.bin",
        size_mb: 466,
        description: "バランス型",
    },
    ModelInfo {
        file_name: "ggml-medium.bin",
        size_mb: 1500,
        description: "高精度",
    },
    ModelInfo {
        file_name: "ggml-large-v3-turbo.bin",
        size_mb: 1600,
        description: "★推奨（高精度・高速）",
    },
    ModelInfo {
        file_name: "ggml-large-v3.bin",
        size_mb: 3100,
        description: "最高精度（大容量）",
    },
];

/// 既知のモデルファイル名かどうかを判定
fn is_known_model(file_name: &str) -> bool {
    MODELS.iter().any(|m| m.file_name == file_name)
}

/// Ensure the Whisper model file exists, downloading it if necessary.
/// If the configured model is not found and is a known model, prompts the user
/// to confirm download or select a different model interactively.
pub fn ensure_model_exists(model_path: &str) -> Result<String> {
    let path = Path::new(model_path);
    if path.exists() {
        return Ok(model_path.to_string());
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .context("Invalid model path")?;

    if !is_known_model(file_name) {
        bail!(
            "モデルファイル '{}' が見つかりません。\n\
             既知のモデル名ではないため自動ダウンロードできません。\n\
             手動で配置するか、settings.toml の model_path を変更してください。",
            model_path
        );
    }

    // モデルが見つからない → 対話的に選択
    println!();
    println!("========================================");
    println!("  Whisper モデルセットアップ");
    println!("========================================");
    println!();
    println!(
        "設定ファイルで指定されたモデル '{}' が見つかりません。",
        file_name
    );
    println!("ダウンロードするモデルを選択してください。");
    println!();

    let selected = prompt_model_selection(file_name)?;
    let models_dir = path.parent().unwrap_or(Path::new("models"));
    let dest_path = models_dir.join(selected.file_name);
    let dest_str = dest_path.to_string_lossy().to_string();

    // 選択したモデルが既に存在する場合（設定と異なるモデルを選んだケース）
    if dest_path.exists() {
        println!("モデル '{}' は既に存在します。", selected.file_name);
        return Ok(dest_str);
    }

    let url = format!("{}/{}", HUGGINGFACE_BASE_URL, selected.file_name);
    println!();
    println!(
        "ダウンロード開始: {} (約{}MB)",
        selected.file_name, selected.size_mb
    );
    println!("URL: {}", url);
    println!();

    // ディレクトリ作成
    fs::create_dir_all(models_dir)
        .with_context(|| format!("ディレクトリ作成失敗: {}", models_dir.display()))?;

    let part_path = format!("{}.part", dest_str);
    // 既知サイズの120%を上限にして異常な肥大化を防ぐ
    let max_bytes = (selected.size_mb as u64) * 1_048_576 * 12 / 10;
    if let Err(e) = download_file(&url, &part_path, max_bytes) {
        let _ = fs::remove_file(&part_path);
        return Err(e);
    }

    fs::rename(&part_path, &dest_str)
        .with_context(|| format!("リネーム失敗: {} -> {}", part_path, dest_str))?;

    println!();
    println!("ダウンロード完了: {}", dest_str);
    info!("モデルダウンロード完了: {}", dest_str);

    Ok(dest_str)
}

/// 対話的にモデルを選択させる
fn prompt_model_selection(configured_name: &str) -> Result<&'static ModelInfo> {
    // 設定ファイルのモデルをデフォルトにする
    let default_index = MODELS
        .iter()
        .position(|m| m.file_name == configured_name)
        .unwrap_or(4); // 見つからなければ large-v3-turbo

    println!("  # | モデル名                    | サイズ   | 説明");
    println!("----+-----------------------------+----------+--------------------");
    for (i, model) in MODELS.iter().enumerate() {
        let marker = if i == default_index { " *" } else { "  " };
        println!(
            "  {} | {:<27} | {:>5}MB  | {}{}",
            i + 1,
            model.file_name,
            model.size_mb,
            model.description,
            marker,
        );
    }
    println!();
    println!("  * = settings.toml で設定済みのモデル");
    println!();

    loop {
        print!(
            "番号を入力してください [1-{}, デフォルト={}]: ",
            MODELS.len(),
            default_index + 1,
        );
        io::stdout().flush()?;

        let mut input = String::new();
        let bytes_read = io::stdin()
            .lock()
            .read_line(&mut input)
            .context("入力の読み取りに失敗")?;

        // EOF（非対話環境）→ デフォルトを使用
        if bytes_read == 0 {
            info!("stdin EOF: デフォルトモデルを選択");
            return Ok(&MODELS[default_index]);
        }

        let input = input.trim();

        // Enterのみ → デフォルト
        if input.is_empty() {
            return Ok(&MODELS[default_index]);
        }

        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= MODELS.len() => {
                return Ok(&MODELS[n - 1]);
            }
            _ => {
                println!("1〜{} の番号を入力してください。", MODELS.len());
            }
        }
    }
}

fn download_file(url: &str, dest: &str, max_bytes: u64) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut response = client
        .get(url)
        .send()
        .context("HuggingFaceへの接続に失敗")?;

    if !response.status().is_success() {
        bail!("ダウンロード失敗: HTTP {}", response.status());
    }

    let total_size = response.content_length();
    if let Some(total) = total_size {
        anyhow::ensure!(
            total <= max_bytes,
            "ダウンロードサイズが上限を超えています: {} bytes (上限: {} bytes)",
            total,
            max_bytes
        );
    }

    let mut file = fs::File::create(dest).with_context(|| format!("ファイル作成失敗: {}", dest))?;

    let mut downloaded: u64 = 0;
    let mut last_progress: u64 = 0;
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = response.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        file.write_all(&buffer[..bytes_read])?;
        downloaded += bytes_read as u64;

        anyhow::ensure!(
            downloaded <= max_bytes,
            "ダウンロードサイズが上限を超えました: {} bytes (上限: {} bytes)",
            downloaded,
            max_bytes
        );

        if downloaded - last_progress >= PROGRESS_INTERVAL_BYTES {
            last_progress = downloaded;
            if let Some(total) = total_size {
                let percent = (downloaded as f64 / total as f64) * 100.0;
                print!(
                    "\r  ダウンロード中... {:.0}MB / {:.0}MB ({:.1}%)",
                    downloaded as f64 / 1_048_576.0,
                    total as f64 / 1_048_576.0,
                    percent
                );
            } else {
                print!(
                    "\r  ダウンロード中... {:.0}MB",
                    downloaded as f64 / 1_048_576.0
                );
            }
            io::stdout().flush()?;
        }
    }

    file.flush()?;
    println!();
    Ok(())
}
