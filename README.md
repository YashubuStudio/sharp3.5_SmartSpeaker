# Smart Speaker

Rust製のローカル動作型スマートスピーカー。ウェイクワード検出、音声認識（STT）、LLM応答生成、音声合成（TTS）を統合した音声アシスタントです。

## アーキテクチャ

```
マイク入力 → Rustpotter(ウェイクワード検出) → 音声録音 → Whisper(STT)
    → Ollama(LLM) → VOICEVOX(TTS) → スピーカー出力
```

| コンポーネント | 技術 | 役割 |
|---|---|---|
| ウェイクワード | Rustpotter 3.0 | ローカルウェイクワード検出 |
| 音声認識 | Whisper v3 (whisper.cpp) | 音声→テキスト変換 |
| LLM | Ollama | テキスト応答生成 |
| 音声合成 | VOICEVOX | テキスト→音声変換 |

## Quick Start（バイナリリリース）

1. [Releases](../../releases) から ZIP をダウンロード
   - **CPU版**: CUDA不要、どのPCでも動作
   - **CUDA版**: NVIDIA GPU搭載PCで高速動作（**NVIDIAドライバー 570.x以上**が必要）

2. ZIPを展開して `setup.bat` を実行（Ollama等を自動インストール）

3. [VOICEVOX](https://voicevox.hiroshiba.jp/) をインストール・起動

4. `run.bat` を実行

Whisperモデルは初回起動時に自動ダウンロードされます（約1.5GB）。

## Building from Source

### 前提条件

- Rust 1.70+
- [Ollama](https://ollama.com/)
- [VOICEVOX](https://voicevox.hiroshiba.jp/)

### セットアップ

```powershell
# 依存サービスのセットアップ
setup.bat

# CPU版ビルド（CUDA不要）
cargo build --release

# 実行
cargo run --release
```

### CUDA版ビルド

NVIDIA GPUで高速なWhisper推論を行う場合：

1. [CUDA Toolkit](https://developer.nvidia.com/cuda-toolkit) をインストール
2. `.cargo/config.toml.example` を `.cargo/config.toml` にコピーし、GPUアーキテクチャを設定
3. ビルド:

```powershell
cargo build --release --features cuda
```

開発時は `dev-run.bat` で自動的にCUDA版をビルド・起動できます。

## 設定

`config/settings.toml` で各種設定を変更できます：

| セクション | 主な設定 |
|---|---|
| `[audio]` | サンプルレート、無音検出閾値、録音最大時間 |
| `[wakeword]` | ウェイクワードファイルパス、検出閾値 |
| `[stt]` | Whisperモデルパス、認識言語 |
| `[llm]` | Ollamaエンドポイント、モデル名、システムプロンプト |
| `[tts]` | VOICEVOXエンドポイント、話者ID、話速 |
| `[rag]` | RAG機能の有効/無効、ナレッジディレクトリ |

## カスタムウェイクワード

デフォルトのウェイクワードを変更したい場合：

1. [Rustpotter Model Creator](https://givimad.github.io/rustpotter-create-model-demo/) にアクセス
2. 任意のウェイクワードで `.rpw` ファイルを作成（3回以上録音推奨）
3. プロジェクトルートに配置
4. `config/settings.toml` の `wakeword_path` を更新

## RAG（検索拡張生成）

ナレッジファイルを `data/knowledge/` に配置すると、LLMの回答にローカル知識を活用できます。

- 対応形式: JSON配列、テキストファイル（空行区切り）
- 有効化: `config/settings.toml` で `[rag] enabled = true`
- 要: `ollama pull nomic-embed-text`

## ハードウェア要件

| 項目 | 最小 | 推奨 |
|---|---|---|
| RAM | 8GB | 16GB+ |
| ストレージ | 5GB | 10GB+ |
| GPU (CUDA版) | NVIDIA GTX 1060+ | RTX 3060+ |
| NVIDIAドライバー (CUDA版) | 570.x以上 | 最新版推奨 |
| マイク | 任意 | ノイズキャンセリング付き |

## License

[MIT](LICENSE)
