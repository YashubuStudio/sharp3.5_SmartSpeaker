use anyhow::Result;
use log::{info, warn};
use reqwest::blocking::Client;
use rodio::Decoder;
use std::io::Cursor;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// LLMストリーミング接続タイムアウト（秒）
/// 初回のモデルロードで時間がかかるGPU環境を考慮
const LLM_CONNECT_TIMEOUT_SECS: u64 = 120;
/// TTS APIリクエストタイムアウト（秒）
const TTS_REQUEST_TIMEOUT_SECS: u64 = 60;
/// TTS API接続タイムアウト（秒）
const TTS_CONNECT_TIMEOUT_SECS: u64 = 10;
/// LLMレスポンスの最大文字数（パイプライン側）
const MAX_RESPONSE_LENGTH: usize = 50_000;

use crate::audio::AudioPlayback;
use crate::llm::OllamaLlm;
use crate::rag::RagEngine;
use crate::tts::{synthesize_with_client, VoicevoxTts};

use super::detect_intent;
use super::sentence_splitter::SentenceSplitter;
use super::Intent;

/// ストリーミングパイプライン用の事前生成済みHTTPクライアント
pub struct PipelineClients {
    /// LLMストリーミング用クライアント（read timeoutなし）
    pub llm_client: Client,
    /// TTS合成用クライアント
    pub tts_client: Client,
}

impl PipelineClients {
    /// パイプライン用HTTPクライアントを生成
    pub fn new() -> Result<Self> {
        let llm_client = Client::builder()
            .connect_timeout(Duration::from_secs(LLM_CONNECT_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new());

        let tts_client = Client::builder()
            .timeout(Duration::from_secs(TTS_REQUEST_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(TTS_CONNECT_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|_| Client::new());

        Ok(Self {
            llm_client,
            tts_client,
        })
    }
}

/// LLM→TTS→Playback をパイプライン化してストリーミング処理する
///
/// LLMが最初の文を生成した時点で即座にTTS合成→再生を開始し、
/// 残りの文はバックグラウンドで並行処理する。
pub fn process_command_streaming(
    command: &str,
    llm: &OllamaLlm,
    tts: &VoicevoxTts,
    playback: &AudioPlayback,
    rag: &mut Option<RagEngine>,
    stt_duration: Duration,
    clients: &PipelineClients,
) -> Result<()> {
    let pipeline_start = Instant::now();

    println!(">>> Processing: \"{}\"", command);

    // インテント判定: ファクト保存ならLLMパイプラインをスキップ
    match detect_intent(command) {
        Intent::SaveFact(fact) => {
            if let Some(ref mut engine) = rag {
                engine.save_fact(&fact)?;
                play_fixed_response("覚えました", tts, playback, &clients.tts_client)?;
            } else {
                play_fixed_response("記憶機能が無効です", tts, playback, &clients.tts_client)?;
            }
            return Ok(());
        }
        Intent::Query => { /* 既存フローへ */ }
    }

    // RAGコンテキスト取得（従来通り同期）
    let augmented_prompt = if let Some(ref mut rag_engine) = rag {
        match rag_engine.retrieve_context(command) {
            Ok(context) if !context.is_empty() => {
                info!("RAG: コンテキスト取得成功");
                format!(
                    "以下の参考情報を踏まえて回答してください:\n\n{}\n\n質問: {}",
                    context, command
                )
            }
            Ok(_) => command.to_string(),
            Err(e) => {
                warn!("RAGコンテキスト取得に失敗: {}", e);
                command.to_string()
            }
        }
    } else {
        command.to_string()
    };

    // チャネル作成
    let (sentence_tx, sentence_rx) = mpsc::channel::<String>();
    let (audio_tx, audio_rx) = mpsc::channel::<Vec<u8>>();

    // --- Thread 1: LLMストリーミング → 文分割 → sentence_tx ---
    let llm_prompt = augmented_prompt.clone();
    // OllamaLlm はSendではないため、ストリーミング読み取りに必要な情報を複製
    let llm_endpoint = llm.endpoint().to_string();
    let llm_model = llm.model().to_string();
    let llm_system_prompt = llm.system_prompt().to_string();
    // Client は Send + Sync なのでクローンしてスレッドに渡す
    let llm_client = clients.llm_client.clone();

    let llm_handle = thread::spawn(move || -> Result<String> {
        #[derive(serde::Serialize)]
        struct GenerateRequest {
            model: String,
            prompt: String,
            system: String,
            stream: bool,
        }

        let request = GenerateRequest {
            model: llm_model,
            prompt: llm_prompt,
            system: llm_system_prompt,
            stream: true,
        };

        let url = format!("{}/api/generate", llm_endpoint);
        let response = llm_client.post(&url).json(&request).send()?;

        if !response.status().is_success() {
            anyhow::bail!("Ollama APIエラー: ステータスコード {}", response.status());
        }

        let reader = std::io::BufReader::new(response);
        let mut splitter = SentenceSplitter::new();
        let mut full_response = String::new();
        let mut first_sentence = true;

        use std::io::BufRead;
        for line_result in reader.lines() {
            match line_result {
                Ok(line) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<crate::llm::StreamChunk>(&line) {
                        Ok(chunk) => {
                            full_response.push_str(&chunk.response);

                            // レスポンスサイズ制限
                            if full_response.len() > MAX_RESPONSE_LENGTH {
                                warn!(
                                    "LLMレスポンスがサイズ上限に到達 ({}文字)。切り詰めます。",
                                    full_response.len()
                                );
                                break;
                            }

                            // 文分割して送出
                            let sentences = splitter.push(&chunk.response);
                            for sentence in sentences {
                                if first_sentence {
                                    let elapsed = pipeline_start.elapsed();
                                    info!(
                                        "最初の文を送出: {:.2}秒 \"{}\"",
                                        elapsed.as_secs_f32(),
                                        sentence
                                    );
                                    first_sentence = false;
                                }
                                if sentence_tx.send(sentence).is_err() {
                                    // レシーバが切断された（TTS側がエラーで停止など）
                                    return Ok(full_response.trim().to_string());
                                }
                            }

                            if chunk.done {
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("ストリームチャンクのパースに失敗: {}", e);
                            continue;
                        }
                    }
                }
                Err(e) => {
                    warn!("ストリーム読み取りエラー（取得済みテキストで続行）: {}", e);
                    break;
                }
            }
        }

        // 残りのバッファをフラッシュ
        if let Some(remaining) = splitter.flush() {
            let _ = sentence_tx.send(remaining);
        }

        // sentence_tx はスコープ終了でドロップされ、TTS側のrecvがエラーを返す
        Ok(full_response.trim().to_string())
    });

    // --- Thread 2: TTS合成 → audio_tx ---
    let tts_endpoint = tts.endpoint().to_string();
    let tts_speaker_id = tts.speaker_id();
    let tts_speed = tts.speed();
    let tts_client = clients.tts_client.clone();

    let tts_handle = thread::spawn(move || {
        while let Ok(sentence) = sentence_rx.recv() {
            info!("TTS合成中: \"{}\"", sentence);
            let tts_start = Instant::now();

            match synthesize_with_client(
                &tts_client,
                &tts_endpoint,
                tts_speaker_id,
                tts_speed,
                &sentence,
            ) {
                Ok(wav_data) => {
                    let tts_time = tts_start.elapsed();
                    info!(
                        "TTS合成完了: {:.2}秒 ({} bytes)",
                        tts_time.as_secs_f32(),
                        wav_data.len()
                    );
                    if audio_tx.send(wav_data).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    warn!("TTS合成エラー（スキップ）: {}", e);
                    continue;
                }
            }
        }
    });

    // --- Main Thread: Playback ---
    let sink = playback.create_sink()?;
    let mut segment_count = 0;

    while let Ok(wav_data) = audio_rx.recv() {
        match Decoder::new(Cursor::new(wav_data)) {
            Ok(source) => {
                sink.append(source);
                segment_count += 1;
                info!("再生キューに追加: セグメント {}", segment_count);
            }
            Err(e) => {
                warn!("WAVデコードエラー（スキップ）: {}", e);
                continue;
            }
        }
    }

    // 全セグメントの再生完了を待機
    sink.sleep_until_end();

    let total_time = stt_duration + pipeline_start.elapsed();
    info!(
        "パイプライン完了: {:.2}秒 (STT: {:.2}秒 + 処理再生: {:.2}秒, {}セグメント)",
        total_time.as_secs_f32(),
        stt_duration.as_secs_f32(),
        pipeline_start.elapsed().as_secs_f32(),
        segment_count
    );

    // スレッドの完了を待機
    let full_response = llm_handle
        .join()
        .map_err(|_| anyhow::anyhow!("LLMスレッドがパニック"))?;
    tts_handle
        .join()
        .map_err(|_| anyhow::anyhow!("TTSスレッドがパニック"))?;

    let response_text = match full_response {
        Ok(text) => text,
        Err(e) => {
            return Err(e);
        }
    };

    println!(">>> Response: \"{}\"", response_text);

    // 会話を保存
    if let Some(ref mut rag_engine) = rag {
        if let Err(e) = rag_engine.save_conversation(command, &response_text) {
            warn!("会話の保存に失敗: {:?}", e);
        }
    }

    println!();
    Ok(())
}

/// 固定テキストをTTS合成して再生する（LLMをスキップする短縮パス）
fn play_fixed_response(
    text: &str,
    tts: &VoicevoxTts,
    playback: &AudioPlayback,
    client: &Client,
) -> Result<()> {
    println!(">>> Response: \"{}\"", text);

    let wav_data = synthesize_with_client(
        client,
        tts.endpoint(),
        tts.speaker_id(),
        tts.speed(),
        text,
    )?;

    let sink = playback.create_sink()?;
    match Decoder::new(Cursor::new(wav_data)) {
        Ok(source) => {
            sink.append(source);
            sink.sleep_until_end();
        }
        Err(e) => {
            warn!("WAVデコードエラー: {}", e);
        }
    }

    println!();
    Ok(())
}
