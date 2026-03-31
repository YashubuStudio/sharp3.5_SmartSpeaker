use log::debug;

/// 録音状態の管理
pub struct RecordingState {
    pub samples: Vec<f32>,
    pub is_recording: bool,
    pub speech_detected: bool,
    consecutive_silence: usize,
    silence_samples_threshold: usize,
    max_samples: usize,
    silence_threshold: f32,
    pub current_level: f32,
    // 無音検出改善用フィールド
    /// 平滑化されたRMS
    smoothed_rms: f32,
    /// 平滑化係数（0.1が推奨）
    smoothing_alpha: f32,
    /// ノイズフロア（キャリブレーション後に設定）
    noise_floor: f32,
    /// キャリブレーション完了フラグ
    calibration_complete: bool,
    /// キャリブレーション期間（サンプル数）
    calibration_duration: usize,
    /// キャリブレーション中のRMS合計
    calibration_rms_sum: f32,
    /// キャリブレーション中のRMSカウント
    calibration_rms_count: usize,
    /// 相対閾値の乗数
    relative_threshold_multiplier: f32,
    /// 連続無音フレーム数（デバウンス用）
    silent_frame_count: usize,
    /// デバウンス閾値
    debounce_frames: usize,
    /// サンプルレート（デバッグログ用）
    #[allow(dead_code)]
    sample_rate: u32,
    /// lookbackサンプル数（キャリブレーション判定で差し引く）
    lookback_len: usize,
}

impl RecordingState {
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            is_recording: false,
            speech_detected: false,
            consecutive_silence: 0,
            silence_samples_threshold: 0,
            max_samples: 0,
            silence_threshold: 0.01,
            current_level: 0.0,
            // 無音検出改善用フィールドの初期化
            smoothed_rms: 0.0,
            smoothing_alpha: 0.1,
            noise_floor: 0.0,
            calibration_complete: false,
            calibration_duration: 0,
            calibration_rms_sum: 0.0,
            calibration_rms_count: 0,
            relative_threshold_multiplier: 3.0,
            silent_frame_count: 0,
            debounce_frames: 3,
            sample_rate: 16000,
            lookback_len: 0,
        }
    }

    pub fn start(
        &mut self,
        lookback_samples: Vec<f32>,
        max_samples: usize,
        silence_samples_threshold: usize,
        silence_threshold: f32,
        sample_rate: u32,
        smoothing_alpha: f32,
        relative_threshold_multiplier: f32,
        calibration_duration: f32,
        debounce_frames: usize,
    ) {
        self.lookback_len = lookback_samples.len();
        self.samples = lookback_samples;
        self.is_recording = true;
        self.speech_detected = false;
        self.consecutive_silence = 0;
        self.silence_samples_threshold = silence_samples_threshold;
        self.max_samples = max_samples;
        self.silence_threshold = silence_threshold;
        self.current_level = 0.0;
        // 無音検出改善用フィールドのリセット
        self.smoothed_rms = 0.0;
        self.smoothing_alpha = smoothing_alpha;
        self.noise_floor = 0.0;
        self.calibration_complete = false;
        self.calibration_duration = (calibration_duration * sample_rate as f32) as usize;
        self.calibration_rms_sum = 0.0;
        self.calibration_rms_count = 0;
        self.relative_threshold_multiplier = relative_threshold_multiplier;
        self.silent_frame_count = 0;
        self.debounce_frames = debounce_frames;
        self.sample_rate = sample_rate;
    }

    pub fn stop(&mut self) -> Vec<f32> {
        self.is_recording = false;
        std::mem::take(&mut self.samples)
    }

    pub fn add_samples(&mut self, samples: &[f32]) {
        if !self.is_recording {
            return;
        }

        self.samples.extend_from_slice(samples);

        // RMS計算
        if !samples.is_empty() {
            let frame_rms =
                (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();

            // 指数移動平均によるRMS平滑化
            if self.smoothed_rms == 0.0 {
                self.smoothed_rms = frame_rms;
            } else {
                self.smoothed_rms = self.smoothing_alpha * frame_rms
                    + (1.0 - self.smoothing_alpha) * self.smoothed_rms;
            }

            self.current_level = self.smoothed_rms;

            // キャリブレーション期間中
            if !self.calibration_complete {
                self.calibration_rms_sum += frame_rms;
                self.calibration_rms_count += 1;

                // キャリブレーション完了判定（サンプル数ベース、lookback分を差し引く）
                let recorded_after_lookback =
                    self.samples.len().saturating_sub(self.lookback_len);
                if recorded_after_lookback >= self.calibration_duration {
                    if self.calibration_rms_count > 0 {
                        self.noise_floor =
                            self.calibration_rms_sum / self.calibration_rms_count as f32;
                        // ノイズフロアの最小値を設定（極端に静かな環境対策）
                        self.noise_floor = self.noise_floor.max(0.001);
                    } else {
                        self.noise_floor = self.silence_threshold;
                    }
                    self.calibration_complete = true;

                    let effective_threshold =
                        self.noise_floor * self.relative_threshold_multiplier;
                    debug!(
                        "Noise floor calibration complete: {:.4}, effective threshold: {:.4}",
                        self.noise_floor, effective_threshold
                    );
                }
                return; // キャリブレーション中は無音判定しない
            }

            // キャリブレーション後：相対閾値による判定
            let effective_threshold = self.noise_floor * self.relative_threshold_multiplier;

            if frame_rms >= effective_threshold {
                // 発話検出
                self.speech_detected = true;
                self.consecutive_silence = 0;
                self.silent_frame_count = 0;
            } else if self.speech_detected {
                // 無音フレームのデバウンス処理
                self.silent_frame_count += 1;

                // 連続した無音フレームがデバウンス閾値を超えたら無音としてカウント
                if self.silent_frame_count >= self.debounce_frames {
                    self.consecutive_silence += samples.len();
                }
            }
        }
    }

    pub fn should_stop(&self) -> bool {
        if !self.is_recording {
            return true;
        }
        if self.samples.len() >= self.max_samples {
            return true;
        }
        if self.speech_detected && self.consecutive_silence >= self.silence_samples_threshold {
            return true;
        }
        false
    }
}
