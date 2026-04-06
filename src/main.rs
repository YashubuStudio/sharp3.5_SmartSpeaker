mod audio;
mod config;
mod llm;
mod pipeline;
mod rag;
mod stt;
mod tts;
mod wakeword;
mod webserver;

use anyhow::Result;
use log::{error, info, warn};

use audio::{AudioCapture, AudioPlayback};
use config::Config;
use llm::OllamaLlm;
use pipeline::PipelineClients;
use rag::RagEngine;
use std::sync::{Arc, Mutex};
use stt::WhisperStt;
use tts::VoicevoxTts;
use wakeword::WakewordDetector;

fn main() -> Result<()> {
    // ログ初期化
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Smart Speaker 起動");

    // 設定ファイル読み込み
    let config = Config::load("config/settings.toml")?;
    config.validate()?;
    info!("設定読み込み・バリデーション完了");

    // 各コンポーネントの初期化とヘルスチェック
    let llm = Arc::new(OllamaLlm::new(&config.llm)?);
    if !llm.health_check()? {
        anyhow::bail!(
            "Ollamaサーバーに接続できません。Ollamaが起動していることを確認してください。"
        );
    }
    info!("Ollama接続OK");

    let tts = Arc::new(VoicevoxTts::new(&config.tts)?);
    if !tts.health_check()? {
        anyhow::bail!(
            "VOICEVOXサーバーに接続できません。VOICEVOXが起動していることを確認してください。"
        );
    }
    info!("VOICEVOX接続OK");

    let actual_model_path = stt::model_downloader::ensure_model_exists(&config.stt.model_path)?;
    if actual_model_path != config.stt.model_path {
        info!(
            "選択されたモデル: {} (設定: {})",
            actual_model_path, config.stt.model_path
        );
        update_settings_model_path("config/settings.toml", &actual_model_path);
    }
    let stt_config = config::SttConfig {
        model_path: actual_model_path,
        language: config.stt.language.clone(),
        use_gpu: config.stt.use_gpu,
        flash_attn: config.stt.flash_attn,
    };
    let stt = WhisperStt::new(&stt_config)?;
    info!("Whisper初期化OK");

    let mut wakeword_detector = WakewordDetector::new(&config.wakeword)?;
    info!("ウェイクワード検出器初期化OK (Rustpotter)");

    let capture = AudioCapture::new(
        config.audio.sample_rate,
        config.audio.input_gain,
        config.audio.smoothing_alpha,
        config.audio.relative_threshold_multiplier,
        config.audio.calibration_duration,
        config.audio.debounce_frames,
    )?;
    let playback = AudioPlayback::new()?;
    info!("オーディオデバイス初期化OK");

    // RAGエンジン初期化
    let rag_engine = if config.rag.enabled {
        match RagEngine::new(&config.rag, &config.llm.endpoint, &config.llm.model) {
            Ok(mut engine) => {
                match engine.index_knowledge() {
                    Ok(count) => info!("RAG初期化OK: {}件のナレッジをインデックス", count),
                    Err(e) => warn!("ナレッジインデックスに失敗: {}", e),
                }
                Some(engine)
            }
            Err(e) => {
                warn!("RAGエンジンの初期化に失敗（RAGなしで続行）: {}", e);
                None
            }
        }
    } else {
        info!("RAG: 無効");
        None
    };
    let rag_engine = Arc::new(Mutex::new(rag_engine));

    // パイプライン用HTTPクライアントを事前生成（毎回生成のオーバーヘッドを回避）
    let pipeline_clients = PipelineClients::new()?;
    info!("パイプラインHTTPクライアント初期化OK");

    // Web UI / API サーバー起動（ローカルネットワーク配信用）
    let _web_server = if config.web.enabled {
        Some(webserver::start_web_server(
            &config.web,
            webserver::WebApiState {
                llm: Arc::clone(&llm),
                tts: Arc::clone(&tts),
                rag: Arc::clone(&rag_engine),
            },
        )?)
    } else {
        info!("Web UI / API: 無効");
        None
    };

    println!();
    println!("========================================");
    println!("  Smart Speaker Ready!");
    println!("  Wakeword file: {}", config.wakeword.wakeword_path);
    if rag_engine.lock().map(|g| g.is_some()).unwrap_or(false) {
        println!("  RAG: enabled");
    }
    if config.web.enabled {
        println!(
            "  Web UI: http://{}:{}/",
            config.web.bind_host, config.web.port
        );
    }
    println!("========================================");

    // メインループ
    loop {
        // ウェイクワード待機（Rustpotter）
        match wakeword_detector.wait_for_wakeword(&capture) {
            Ok(result) => {
                info!(
                    "ウェイクワード \"{}\" 検出 (score: {:.2})",
                    result.keyword, result.score
                );

                // コマンドを録音
                println!(">>> Listening for your command...");
                match get_voice_command(&config, &capture, &stt) {
                    Ok(Some((cmd, stt_duration))) => {
                        // LLM応答をストリーミングパイプラインで生成・再生
                        let mut rag_guard = match rag_engine.lock() {
                            Ok(guard) => guard,
                            Err(_) => {
                                error!("RAGロック取得に失敗");
                                continue;
                            }
                        };
                        if let Err(e) = pipeline::process_command_streaming(
                            &cmd,
                            &llm,
                            &tts,
                            &playback,
                            &mut rag_guard,
                            stt_duration,
                            &pipeline_clients,
                        ) {
                            error!("処理エラー: {}", e);
                        }
                    }
                    Ok(None) => {
                        warn!("コマンドを認識できませんでした。");
                    }
                    Err(e) => {
                        error!("録音エラー: {}", e);
                    }
                }
            }
            Err(e) => {
                error!("ウェイクワード検出エラー: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

/// settings.toml の model_path を更新する
///
/// TOML全体をパースし直すのではなく、行単位の置換で他の設定やコメントを保持する。
fn update_settings_model_path(settings_path: &str, new_model_path: &str) {
    let content = match std::fs::read_to_string(settings_path) {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "settings.toml の読み込みに失敗（モデルパス更新スキップ）: {}",
                e
            );
            return;
        }
    };

    let mut updated_lines = Vec::new();
    let mut in_stt_section = false;
    for line in content.lines() {
        let trimmed = line.trim_start();
        // セクションヘッダの追跡
        if trimmed.starts_with('[') {
            in_stt_section = trimmed.starts_with("[stt]");
        }
        if in_stt_section && trimmed.starts_with("model_path") && trimmed.contains('=') {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            updated_lines.push(format!("{}model_path = \"{}\"", indent, new_model_path));
        } else {
            updated_lines.push(line.to_string());
        }
    }

    let new_content = updated_lines.join("\n");
    // 元ファイルが改行で終わっていた場合は維持
    let new_content = if content.ends_with('\n') {
        format!("{}\n", new_content)
    } else {
        new_content
    };

    match std::fs::write(settings_path, &new_content) {
        Ok(()) => info!(
            "settings.toml の model_path を更新しました: {}",
            new_model_path
        ),
        Err(e) => warn!("settings.toml の書き込みに失敗: {}", e),
    }
}

/// 音声コマンドを取得
fn get_voice_command(
    config: &Config,
    capture: &AudioCapture,
    stt: &WhisperStt,
) -> Result<Option<(String, std::time::Duration)>> {
    let audio_data = capture.record_with_feedback(
        config.audio.max_record_seconds,
        config.audio.silence_threshold,
        config.audio.silence_duration,
    )?;

    if audio_data.len() < (config.audio.sample_rate as usize / 2) {
        return Ok(None);
    }

    let start = std::time::Instant::now();
    info!("音声認識中...");
    let text = stt.transcribe(&audio_data)?;
    let stt_duration = start.elapsed();
    info!("STT完了: {:.2}秒", stt_duration.as_secs_f32());

    let text = text.trim().to_string();

    if text.is_empty() {
        return Ok(None);
    }

    println!(">>> You said: \"{}\"", text);
    Ok(Some((text, stt_duration)))
}
