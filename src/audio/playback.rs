use anyhow::Result;
use log::info;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use thiserror::Error;

/// 音声再生に関するエラー
#[derive(Debug, Error)]
pub enum PlaybackError {
    #[error("出力デバイスの初期化に失敗: {0}")]
    DeviceError(String),

    #[error("再生中にエラーが発生: {0}")]
    PlayError(String),
}

/// スピーカーへの音声再生を管理
pub struct AudioPlayback {
    _stream: OutputStream,
    handle: OutputStreamHandle,
}

impl AudioPlayback {
    /// デフォルトの出力デバイスでAudioPlaybackを初期化
    pub fn new() -> Result<Self> {
        let (stream, handle) = OutputStream::try_default()
            .map_err(|e| PlaybackError::DeviceError(e.to_string()))?;

        info!("音声再生デバイスを初期化しました");

        Ok(Self {
            _stream: stream,
            handle,
        })
    }

    /// 空のSinkを作成（ストリーミングパイプライン用）
    ///
    /// appendでWAVセグメントを順次追加し、キュー再生する用途。
    pub fn create_sink(&self) -> Result<Sink> {
        let sink = Sink::try_new(&self.handle)
            .map_err(|e| PlaybackError::PlayError(e.to_string()))?;
        Ok(sink)
    }

}
