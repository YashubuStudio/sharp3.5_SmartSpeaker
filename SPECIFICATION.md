# Smart Speaker 仕様書

## 1. 概要

Rust製のローカル動作型スマートスピーカー。ウェイクワード検出、音声認識（STT）、LLM応答生成、音声合成（TTS）を統合し、ストリーミングパイプラインにより低遅延な音声応答を実現する。

## 2. システムアーキテクチャ

### 2.1 処理フロー

```
マイク常時キャプチャ（リングバッファ）
       │
       ▼
Rustpotter ウェイクワード検出（フレーム単位ループ）
       │ 検出
       ▼
音声録音（VADによる発話終了検出）
       │
       ▼
意図分類（プレフィックスマッチ）
       │
       ├─ SaveFact → RAG保存 → 固定応答TTS再生
       │
       └─ Query → RAGコンテキスト取得 → ストリーミングパイプライン
                                              │
                              ┌────────────────┼────────────────┐
                              ▼                ▼                ▼
                         LLMスレッド       TTSスレッド      メインスレッド
                         (Ollama)        (VOICEVOX)       (rodio再生)
                              │                │                │
                         NDJSON解析        文→WAV合成      WAVキュー再生
                         → 文分割          → audio_tx      → Sink追加
                         → sentence_tx                      → 再生完了待ち
```

### 2.2 スレッドモデル

| スレッド | 役割 | 通信 |
|---------|------|------|
| メインスレッド | 初期化、イベントループ、音声再生キュー管理 | audio_rx → Sink |
| LLMスレッド | Ollama ストリーミング応答 → 文分割 | sentence_tx |
| TTSスレッド | VOICEVOX 音声合成 | sentence_rx → audio_tx |

スレッド間通信は `std::sync::mpsc` チャネルを使用。

### 2.3 モジュール構成

```
src/
├── main.rs                      # エントリポイント、初期化、メインループ
├── config.rs                    # 設定ファイル読み込み・バリデーション
├── audio/
│   ├── mod.rs                   # モジュールエクスポート
│   ├── capture.rs               # マイク入力（リングバッファ、リサンプリング）
│   ├── playback.rs              # スピーカー出力（rodio）
│   ├── ring_buffer.rs           # リングバッファ実装
│   └── vad.rs                   # 音声区間検出（VAD）
├── wakeword/
│   ├── mod.rs                   # モジュールエクスポート
│   └── detector.rs              # Rustpotter連携、前処理パイプライン
├── stt/
│   ├── mod.rs                   # モジュールエクスポート
│   ├── whisper.rs               # whisper.cpp連携、VAD前処理
│   └── model_downloader.rs      # Whisperモデル自動ダウンロード
├── llm/
│   ├── mod.rs                   # モジュールエクスポート
│   └── ollama.rs                # Ollama API連携（ストリーミング対応）
├── tts/
│   ├── mod.rs                   # モジュールエクスポート
│   └── voicevox.rs              # VOICEVOX API連携
├── rag/
│   ├── mod.rs                   # RAGエンジン（インデックス、検索、保存）
│   ├── embedder.rs              # Ollama埋め込みAPI連携
│   └── store.rs                 # ベクトルストア（インメモリ＋JSONL永続化）
└── pipeline/
    ├── mod.rs                   # 意図分類（Intent Detection）
    ├── sentence_splitter.rs     # 日本語文境界検出
    └── streaming.rs             # ストリーミングパイプライン実装
```

## 3. コンポーネント仕様

### 3.1 音声キャプチャ (`audio/capture.rs`)

マイクからの常時音声キャプチャを管理する。

**初期化パラメータ:**

| パラメータ | 型 | 説明 |
|-----------|-----|------|
| target_sample_rate | u32 | 目標サンプルレート（通常16000Hz） |
| input_gain | f32 | 入力ゲイン倍率 |
| smoothing_alpha | f32 | EMA平滑化係数 |
| relative_threshold_multiplier | f32 | ノイズフロア倍率（無音判定用） |
| calibration_duration | f32 | キャリブレーション秒数 |
| debounce_frames | usize | デバウンスフレーム数 |

**リングバッファ:**

- 容量: 96,000サンプル（48kHzで2秒分）
- ルックバック: 24,000サンプル（0.5秒分、ウェイクワード直前の音声を保持）
- 読み書き: アトミックポインタによるロックフリー実装

**リサンプリング:**

デバイスのネイティブサンプルレートから目標レートへ線形補間でリサンプリング。マルチチャネル入力はモノラルに変換。

**VAD（音声区間検出）:**

- RMSベースのエネルギー検出
- 録音開始時にノイズフロアをキャリブレーション
- 相対閾値: `ノイズフロア × multiplier`
- EMA平滑化で瞬間的なノイズを除外
- デバウンスカウンタで短い無音を無視

**主要メソッド:**

| メソッド | 説明 |
|---------|------|
| `get_samples(n)` | リングバッファから最新n個のサンプルを取得 |
| `record_samples(n)` | ストリーム読み取り（ウェイクワード検出ループ用） |
| `record_with_feedback(max, threshold, duration)` | 視覚フィードバック付き録音（コマンド入力用） |
| `reset_stream_position()` | ストリーム読み取り位置をリセット |

### 3.2 音声再生 (`audio/playback.rs`)

**主要メソッド:**

| メソッド | 説明 |
|---------|------|
| `new()` | デフォルト出力デバイスで初期化 |
| `create_sink()` | ストリーミング再生用のSinkを作成 |
| `play_wav(data)` | WAVデータを同期再生 |
| `play_wav_async(data)` | WAVデータを非同期再生（Sinkを返す） |

ストリーミングパイプラインでは `create_sink()` で作成したSinkにWAVセグメントを順次追加し、FIFO順序で再生する。

### 3.3 ウェイクワード検出 (`wakeword/detector.rs`)

Rustpotter 3.0を使用したローカルウェイクワード検出。

**設定:**

| 項目 | デフォルト | 説明 |
|------|-----------|------|
| wakeword_path | "wakeword.rpw" | ウェイクワードモデルファイル |
| threshold | 0.35 | 検出閾値（0.0〜1.0） |
| avg_threshold | 0.15 | 平均スコア閾値 |
| min_scores | 3 | 連続検出回数（デフォルト値。config設定で上書き） |

**前処理パイプライン:**

1. **正規化**: ピーク値を28,000（i16の約85%）にスケーリング
2. **VAD**: 無音フレームのゲインを0.1倍に低減（ゼロにはしない）

**ウォームアップ:**

起動後最初の300フレーム（約1秒）は検出をスキップし、誤検出を防止。

**使用禁止設定:**

以下はscore=0問題を引き起こすため使用しない:
- `detector.eager = false`
- `filters.band_pass.enabled = true`
- `filters.gain_normalizer.enabled = true`

### 3.4 音声認識 (`stt/whisper.rs`)

whisper.cpp (whisper-rs) を使用した音声テキスト変換。

**前処理:**

1. VADによる無音区間の除去
   - フレームサイズ: 320サンプル（20ms@16kHz）
   - 音声閾値: RMS > 0.01
   - マージン: 前後5フレーム
   - 最大ギャップ: 10フレーム（それ以下は統合）
2. ピーク正規化（目標: 0.9）

**認識パラメータ:**

| パラメータ | 値 | 説明 |
|-----------|-----|------|
| beam_size | 5 | ビームサーチ幅 |
| patience | 1.0 | ビーム探索の忍耐度 |
| no_speech_prob閾値 | 0.6 | これ以上は非音声として除外 |
| language | "ja" | 認識言語（設定可能） |

### 3.5 Whisperモデル自動ダウンロード (`stt/model_downloader.rs`)

**対応モデル:**

| モデル | サイズ | 説明 |
|-------|--------|------|
| ggml-tiny.bin | 75MB | 最小・最速（テスト用） |
| ggml-base.bin | 142MB | 軽量 |
| ggml-small.bin | 466MB | バランス型 |
| ggml-medium.bin | 1,500MB | 高精度 |
| ggml-large-v3-turbo.bin | 1,600MB | **推奨**（高精度・高速） |
| ggml-large-v3.bin | 3,100MB | 最高精度 |

**動作:**

1. 設定ファイルのモデルパスを確認
2. ファイルが存在すれば使用
3. 存在しなければ対話的選択プロンプトを表示
4. HuggingFaceから `.part` ファイルとしてダウンロード（10MB毎に進捗表示）
5. 完了後にリネーム（アトミック書き込み）

ダウンロード元: `https://huggingface.co/ggerganov/whisper.cpp/resolve/master/{model_name}`

### 3.6 LLM生成 (`llm/ollama.rs`)

Ollama APIを使用したテキスト応答生成。

**API仕様:**

| 項目 | 値 |
|------|-----|
| エンドポイント | `{endpoint}/api/generate` |
| メソッド | POST |
| リクエスト | `{"model", "prompt", "system", "stream"}` |
| レスポンス（ストリーミング） | NDJSON（1行1チャンク） |

**タイムアウト:**

| 種別 | 秒数 |
|------|------|
| 非ストリーミングリクエスト | 120秒 |
| ストリーミング接続 | 30秒 |
| ストリーミング読み取り | 無制限（チャンク間隔が可変のため） |

**安全制限:**

- レスポンス最大長: 50,000文字（超過時は切り捨て）
- 非localhostエンドポイントに対して警告ログを出力

**主要メソッド:**

| メソッド | 説明 |
|---------|------|
| `generate(prompt)` | 非ストリーミング生成（ブロッキング） |
| `generate_stream(prompt, callback)` | ストリーミング生成（チャンク毎にコールバック） |
| `health_check()` | APIヘルスチェック |
| `list_models()` | 利用可能モデル一覧取得 |

### 3.7 音声合成 (`tts/voicevox.rs`)

VOICEVOX APIを使用したテキスト音声変換。

**API仕様（2段階）:**

1. `POST /audio_query?text={text}&speaker={id}` → クエリJSON取得
2. クエリJSONの `speedScale` を設定値で上書き
3. `POST /synthesis?speaker={id}` (body: クエリJSON) → WAVバイト列取得

**タイムアウト:**

| 種別 | 秒数 |
|------|------|
| リクエスト | 60秒 |
| 接続 | 10秒 |

**安全制限:**

- WAVレスポンス最大サイズ: 50MB
- 非localhostエンドポイントに対して警告ログを出力

**スタンドアロン関数:**

`synthesize_with_client(client, endpoint, speaker_id, speed, text)` — ストリーミングスレッドでの使用向け（`VoicevoxTts`はSendを実装しないため）。

### 3.8 RAG (`rag/`)

検索拡張生成（Retrieval-Augmented Generation）。

#### 3.8.1 埋め込み (`rag/embedder.rs`)

| 項目 | 値 |
|------|-----|
| APIエンドポイント | `{endpoint}/api/embed` |
| リクエスト | `{"model", "input"}` |
| レスポンス | `{"embeddings": [[...]]}` |
| 最大次元数 | 8,192 |
| デフォルトモデル | nomic-embed-text |

初回呼び出しで次元数を記録し、以降の一貫性を検証する。

#### 3.8.2 ベクトルストア (`rag/store.rs`)

**永続化形式:** JSONL（1行1ドキュメント、追記書き込み）

**ドキュメント構造:**

| フィールド | 型 | 説明 |
|-----------|-----|------|
| id | String | `{doc_type}_{timestamp}` 形式 |
| doc_type | String | "knowledge" / "fact" / "conversation" |
| content | String | テキスト内容 |
| embedding | Vec\<f32\> | ベクトル表現 |
| timestamp | String | RFC3339タイムスタンプ |
| source_file | Option\<String\> | 元ファイル名（knowledgeのみ） |
| importance | Option\<u8\> | 1-5（fact/conversationのみ） |
| sentiment | Option\<String\> | 感情ラベル（conversationのみ） |

**検索アルゴリズム:**

1. 全ドキュメントとのコサイン類似度を計算
2. 閾値（デフォルト0.3）でフィルタ
3. factドキュメントのスコアに1.2倍のブーストを適用
4. スコア降順ソート → top_k件を返却

#### 3.8.3 RAGエンジン (`rag/mod.rs`)

**ナレッジファイル形式:**

- **JSON**: `[{"title": "...", "content": "..."}, ...]` の配列
- **TXT**: 空行区切りのチャンク

**主要メソッド:**

| メソッド | 説明 |
|---------|------|
| `index_knowledge()` | `data/knowledge/` 内のファイルをインデックス |
| `retrieve_context(query)` | クエリに関連するドキュメントを検索・整形 |
| `save_fact(fact)` | ユーザー指定の事実を保存（importance=3） |
| `save_conversation(query, response)` | 会話をメタデータ付きで保存 |

**セキュリティ:**

- シンボリックリンクの検出・拒否
- パス正規化によるディレクトリトラバーサル防止
- ナレッジファイルが `knowledge/` ディレクトリ内にあることを検証

### 3.9 パイプライン (`pipeline/`)

#### 3.9.1 意図分類 (`pipeline/mod.rs`)

**プレフィックスマッチング方式:**

| プレフィックス | 意図 |
|--------------|------|
| 覚えて | SaveFact |
| メモして | SaveFact |
| 記録して | SaveFact |
| 保存して | SaveFact |
| （その他） | Query |

プレフィックス後の区切り文字（`、`、`。`、半角/全角スペース）を除去し、残りの本文を抽出。本文が空の場合はQueryとして処理。

#### 3.9.2 文分割 (`pipeline/sentence_splitter.rs`)

LLMのストリーミングトークンを文単位に分割する。

**分割ルール:**

| 条件 | 動作 |
|------|------|
| `。` `！` `？` `!` `?` | 句点で分割 |
| `\n` | 改行で即座に分割 |
| `」` `』` `）` `)` | 閉じ括弧が続く場合は括弧まで含めて分割 |
| バッファ > 200文字 | `、` `,` スペースで強制分割。見つからなければ全体を出力 |

#### 3.9.3 ストリーミングパイプライン (`pipeline/streaming.rs`)

**処理フロー:**

1. 意図分類 → SaveFactなら固定応答（"覚えました"）をTTS再生して終了
2. RAGコンテキスト取得（有効時）→ プロンプトに付加
3. 3スレッド並行処理:
   - LLMスレッド: ストリーミング応答 → 文分割 → `sentence_tx`
   - TTSスレッド: `sentence_rx` → VOICEVOX合成 → `audio_tx`
   - メインスレッド: `audio_rx` → Sinkキュー → 再生完了待ち
4. 会話をRAGに保存（有効時）

**エラー処理:**

- TTS合成エラー: 該当文をスキップ（非致命的）
- WAVデコードエラー: 該当セグメントをスキップ（非致命的）
- LLMスレッドパニック: Resultエラーに変換

**パフォーマンス計測:**

- 最初の文の出力までの時間をログ
- パイプライン全体の処理時間をログ
- 再生セグメント数をログ

## 4. 設定ファイル仕様

`config/settings.toml`:

### 4.1 [audio]

| キー | 型 | デフォルト | 説明 |
|------|-----|-----------|------|
| sample_rate | u32 | 16000 | サンプルレート（Hz） |
| max_record_seconds | u32 | 10 | 最大録音時間（秒） |
| silence_threshold | f32 | 0.03 | 無音検出閾値（絶対値） |
| silence_duration | f32 | 1.0 | 無音継続で録音終了（秒） |
| input_gain | f32 | 1.0 | 入力ゲイン倍率 |
| smoothing_alpha | f32 | 0.1 | EMA平滑化係数 |
| relative_threshold_multiplier | f32 | 3.0 | ノイズフロア倍率 |
| calibration_duration | f32 | 0.5 | キャリブレーション時間（秒） |
| debounce_frames | usize | 3 | デバウンスフレーム数 |

### 4.2 [wakeword]

| キー | 型 | デフォルト | 説明 |
|------|-----|-----------|------|
| wakeword_path | String | "wakeword.rpw" | .rpwファイルパス |
| threshold | f32 | 0.35 | 検出閾値 |
| avg_threshold | f32 | 0.15 | 平均スコア閾値 |
| min_scores | usize | 3 | 連続検出回数 |

### 4.3 [stt]

| キー | 型 | デフォルト | 説明 |
|------|-----|-----------|------|
| model_path | String | (必須) | Whisperモデルファイルパス |
| language | String | "ja" | 認識言語 |

### 4.4 [llm]

| キー | 型 | デフォルト | 説明 |
|------|-----|-----------|------|
| endpoint | String | (必須) | OllamaエンドポイントURL |
| model | String | (必須) | 使用モデル名 |
| system_prompt | String | (必須) | システムプロンプト |

### 4.5 [tts]

| キー | 型 | デフォルト | 説明 |
|------|-----|-----------|------|
| endpoint | String | (必須) | VOICEVOXエンドポイントURL |
| speaker_id | i32 | (必須) | 話者ID |
| speed | f32 | (必須) | 話速（0.5〜2.0） |

### 4.6 [rag]

| キー | 型 | デフォルト | 説明 |
|------|-----|-----------|------|
| enabled | bool | false | RAG機能の有効/無効 |
| data_dir | String | "data" | データディレクトリ |
| embedding_model | String | "nomic-embed-text" | 埋め込みモデル名 |
| top_k | usize | 3 | 検索結果の最大数 |
| similarity_threshold | f32 | 0.3 | 類似度閾値 |

## 5. 外部サービスAPI

### 5.1 Ollama

| 用途 | エンドポイント | メソッド |
|------|--------------|---------|
| テキスト生成 | `/api/generate` | POST |
| 埋め込み生成 | `/api/embed` | POST |
| ヘルスチェック | `/api/tags` | GET |
| モデル一覧 | `/api/tags` | GET |

### 5.2 VOICEVOX

| 用途 | エンドポイント | メソッド |
|------|--------------|---------|
| 音声合成クエリ | `/audio_query?text={}&speaker={}` | POST |
| 音声合成 | `/synthesis?speaker={}` | POST |
| ヘルスチェック | `/version` | GET |

## 6. ビルド・配布

### 6.1 Feature Flags

| Feature | 説明 | ビルドコマンド |
|---------|------|--------------|
| cuda (default) | CUDA対応Whisper | `cargo build --release` |
| (なし) | CPU専用Whisper | `cargo build --release --no-default-features` |

### 6.2 配布バリアント

| バリアント | 対象 | GPU依存 |
|-----------|------|---------|
| CPU版 | 全ユーザー | なし |
| GPU版 (CUDA) | NVIDIA GPU搭載PC | CUDA Toolkit 12.x + GPUドライバー |

### 6.3 依存クレート

| カテゴリ | クレート | バージョン |
|---------|---------|-----------|
| オーディオI/O | cpal | 0.15 |
| 音声再生 | rodio | 0.19 |
| ウェイクワード | rustpotter | 3.0 |
| 浮動小数点互換 | half | =2.4.1（ピン固定） |
| 音声認識 | whisper-rs | 0.15 |
| HTTP | reqwest | 0.11 (json, blocking) |
| URLエンコード | urlencoding | 2.1 |
| シリアライズ | serde | 1.0 (derive) |
| JSON | serde_json | 1.0 |
| TOML | toml | 0.8 |
| 時刻 | chrono | 0.4 (serde) |
| エラー | anyhow | 1.0 |
| エラー型 | thiserror | 1.0 |
| ログIF | log | 0.4 |
| ログ実装 | env_logger | 0.10 |

## 7. セキュリティ仕様

### 7.1 ネットワーク

- 全HTTPクライアントにタイムアウト設定（接続・読み取り）
- 非localhostエンドポイントに対するセキュリティ警告
- ストリーミング接続は接続タイムアウトのみ（読み取りタイムアウトなし）

### 7.2 リソース制限

| 制限 | 値 | 対象 |
|------|-----|------|
| LLMレスポンス最大長 | 50,000文字 | ollama.rs, streaming.rs |
| TTS音声最大サイズ | 50MB | voicevox.rs |
| 埋め込み最大次元 | 8,192 | embedder.rs |

### 7.3 ファイルシステム

- シンボリックリンクの検出・拒否（ナレッジファイル読み込み時）
- パス正規化によるディレクトリトラバーサル防止
- 既知モデル名のみダウンロード許可（任意URLからのDLは防止）

## 8. エラー処理方針

### 8.1 初期化フェーズ（致命的エラー）

- 音声デバイスが見つからない
- 設定ファイルのバリデーション失敗
- 外部サービスのヘルスチェック失敗
- Whisperモデルのロード失敗

### 8.2 実行フェーズ（非致命的エラー）

- TTS合成失敗 → 該当文をスキップ
- WAVデコード失敗 → 該当セグメントをスキップ
- RAG検索失敗 → 警告ログ出力、コンテキストなしで続行
- 会話保存失敗 → 警告ログ出力、次のコマンドへ

### 8.3 エラー型一覧

| モジュール | エラー型 | バリアント |
|-----------|---------|-----------|
| audio/capture | CaptureError | NoInputDevice, ConfigError, StreamError, RecordingError |
| audio/playback | PlaybackError | DeviceError, DecodeError, PlayError |
| stt/whisper | SttError | ModelLoadError, TranscriptionError |
| llm/ollama | LlmError | ConnectionError, GenerationError |
| tts/voicevox | TtsError | ConnectionError, AudioQueryError, SynthesisError |

## 9. パフォーマンス特性

### 9.1 レイテンシ目安

| フェーズ | 時間 | 備考 |
|---------|------|------|
| 起動（モデルロード含む） | 3〜12秒 | Whisperモデルサイズ依存 |
| ウェイクワード検出 | リアルタイム | 常時リングバッファから処理 |
| 録音 | 発話長 + 無音待ち | 通常1〜10秒 |
| STT（Whisper推論） | 0.5〜5秒 | モデルサイズ・発話長依存 |
| LLM最初のトークン | モデル依存 | gemma3:4bで約1秒 |
| TTS（1文） | 0.5〜2秒 | 文の長さ依存 |

### 9.2 メモリ使用量

| 項目 | 容量 |
|------|------|
| リングバッファ | 192KB (96,000 × f32) |
| アプリ本体 | 約50MB |
| コマンド処理時 | 追加約20MB |

### 9.3 VRAM使用量

| モデル | VRAM |
|-------|------|
| Whisper large-v3-turbo | 約3GB |
| Whisper medium | 約1.5GB |
| Whisper small | 約0.5GB |
| gemma3:4b (Q4) | 約3GB |
| nomic-embed-text | 約0.3GB |

## 10. テスト

### 10.1 ユニットテスト一覧

| モジュール | テスト数 | 対象 |
|-----------|---------|------|
| pipeline/mod.rs | 9 | 意図分類（全プレフィックス、区切り文字、エッジケース） |
| pipeline/sentence_splitter.rs | 11 | 文境界検出（括弧、句読点、強制分割） |
| rag/mod.rs | 8 | JSON抽出、感情バリデーション、後方互換性 |
| rag/store.rs | 5 | コサイン類似度の計算・エッジケース |
| **合計** | **33** | |

### 10.2 テスト実行

```bash
cargo test
```

E2Eテストはなし（Ollama/VOICEVOX/音声デバイスのモック要）。

## 11. 制限事項

1. **ブロッキングHTTP**: 全HTTP呼び出しが同期（async/awaitなし）
2. **インメモリベクトル検索**: 線形スキャン方式のため約10,000件まで実用的
3. **単一音声**: セッション中の話者ID変更不可（再起動が必要）
4. **認証なし**: ローカルエンドポイント前提（本番利用にはリバースプロキシが必要）
5. **Windows専用**: cpal/rodioはクロスプラットフォームだが、動作検証はWindows環境のみ
6. **定数のハードコード**: タイムアウト値やバッファサイズは定数定義（設定ファイル化可能）
