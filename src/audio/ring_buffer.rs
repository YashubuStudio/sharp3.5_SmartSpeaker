/// リングバッファの容量（2秒分 @ 48kHz = 96000サンプル）
/// デバイスレートが48kHzの場合でも十分な容量を確保
pub const RING_BUFFER_CAPACITY: usize = 96000;

/// リングバッファの内部状態
pub struct AudioCaptureInner {
    pub ring_buffer: Vec<f32>,
    pub write_pos: usize,
    pub total_written: u64,
    /// ストリーミング用の読み取り位置（total_written単位）
    pub stream_read_pos: u64,
}

impl AudioCaptureInner {
    pub fn new() -> Self {
        Self {
            ring_buffer: vec![0.0; RING_BUFFER_CAPACITY],
            write_pos: 0,
            total_written: 0,
            stream_read_pos: 0,
        }
    }

    /// リングバッファにモノラル変換済みサンプルを書き込む
    pub fn write_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            self.ring_buffer[self.write_pos] = sample;
            self.write_pos = (self.write_pos + 1) % RING_BUFFER_CAPACITY;
            self.total_written += 1;
        }
    }

    /// リングバッファから最新のN個のサンプルを読み取る（従来方式）
    pub fn read_latest(&self, num_samples: usize) -> Vec<f32> {
        let num_samples = num_samples.min(RING_BUFFER_CAPACITY);
        let available = (self.total_written as usize).min(RING_BUFFER_CAPACITY);
        let actual_samples = num_samples.min(available);

        if actual_samples == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(actual_samples);
        let start_pos =
            (self.write_pos + RING_BUFFER_CAPACITY - actual_samples) % RING_BUFFER_CAPACITY;

        for i in 0..actual_samples {
            result.push(self.ring_buffer[(start_pos + i) % RING_BUFFER_CAPACITY]);
        }

        result
    }

    /// ストリーミング読み取り用: まだ読み取っていないサンプル数を返す
    pub fn unread_samples(&self) -> usize {
        if self.total_written <= self.stream_read_pos {
            return 0;
        }
        let unread = self.total_written - self.stream_read_pos;
        // リングバッファ容量を超えていたらデータロスト
        unread.min(RING_BUFFER_CAPACITY as u64) as usize
    }

    /// ストリーミング読み取り: 連続した次のN個のサンプルを返す（重複なし）
    pub fn read_stream(&mut self, num_samples: usize) -> Vec<f32> {
        let available = self.unread_samples();
        let to_read = num_samples.min(available);

        if to_read == 0 {
            return Vec::new();
        }

        // 読み取り位置がオーバーライトされた場合、最古の有効位置にジャンプ
        if self.total_written > RING_BUFFER_CAPACITY as u64 {
            let oldest_valid = self.total_written - RING_BUFFER_CAPACITY as u64;
            if self.stream_read_pos < oldest_valid {
                self.stream_read_pos = oldest_valid;
            }
        }

        // シンプルな計算: 論理位置をバッファインデックスに変換
        let buffer_start = (self.stream_read_pos % RING_BUFFER_CAPACITY as u64) as usize;

        let mut result = Vec::with_capacity(to_read);
        for i in 0..to_read {
            result.push(self.ring_buffer[(buffer_start + i) % RING_BUFFER_CAPACITY]);
        }

        // 読み取り位置を進める
        self.stream_read_pos += to_read as u64;

        result
    }

    /// ストリーミング読み取り位置をリセット（現在位置に同期）
    pub fn reset_stream_position(&mut self) {
        self.stream_read_pos = self.total_written;
    }

    /// バッファをクリアしてリセット
    pub fn clear(&mut self) {
        self.ring_buffer.fill(0.0);
        self.write_pos = 0;
        self.total_written = 0;
        self.stream_read_pos = 0;
    }

}
