# パッケージ化・配布

## Completed

- [x] **Phase 1: プロジェクト整理**
  - `sakura.rpw` → `wakeword.rpw` にコピー
  - `settings.toml` の `wakeword_path` 更新
  - `.gitignore` 更新（`*.rpw` 除去、`.cargo/config.toml`・`data/store.jsonl` 追加）
  - `.cargo/config.toml.example` 作成（GPUアーキテクチャ説明付き）
  - `Cargo.toml` に `license`, `readme` メタデータ追加
  - `LICENSE` (MIT) 作成
  - `settings.toml` の `rag.enabled = false` に変更（配布デフォルト）

- [x] **Phase 2: CUDA/CPU feature flag**
  - `[features] default = [], cuda = ["whisper-rs/cuda"]` 追加
  - `whisper-rs` からハードコード `features = ["cuda"]` を除去
  - CPU版: `cargo build --release` / CUDA版: `cargo build --release --features cuda`

- [x] **Phase 3: Whisperモデル自動ダウンロード**
  - `src/stt/model_downloader.rs` 新規作成
  - HuggingFaceから自動DL、`.part` ファイル + リネーム方式
  - 10MB毎の進捗表示
  - `main.rs` に `ensure_model_exists()` 呼び出し追加

- [x] **Phase 4: セットアップスクリプト**
  - `setup.bat` 新規作成（Rust/Ollama自動インストール、VOICEVOX案内）
  - `run.bat` 改修（ハードコードパス除去、バイナリ直接実行対応）
  - `dev-run.bat` 新規作成（CUDA版ビルド＋起動）

- [x] **Phase 5: GitHub Actions CI/CD**
  - `.github/workflows/ci.yml` — PR/push時のcheck/test/clippy/fmt
  - `.github/workflows/release.yml` — タグpushでCPU/CUDA両ZIPビルド＋Release

- [x] **Phase 6-7: README.md**
  - Quick Start（バイナリ向け）、Building from Source、CUDA設定、設定ファイル説明
  - カスタムウェイクワード作成方法、RAG説明、ハードウェア要件

- [x] **Phase 8: 検証**
  - `cargo check --release` CPU版ビルド成功（CUDAなし）
  - `cargo test` 全16テスト通過

## 変更ファイル一覧

| ファイル | 操作 |
|---|---|
| `Cargo.toml` | 編集 (feature flags, メタデータ) |
| `src/main.rs` | 編集 (model_downloader呼び出し) |
| `src/stt/mod.rs` | 編集 (model_downloader追加) |
| `src/stt/model_downloader.rs` | **新規** |
| `config/settings.toml` | 編集 (wakeword_path, rag.enabled) |
| `.gitignore` | 編集 |
| `.cargo/config.toml.example` | **新規** |
| `wakeword.rpw` | **新規** (sakura.rpwのコピー) |
| `run.bat` | 編集 (配布用に改修) |
| `dev-run.bat` | **新規** |
| `setup.bat` | **新規** |
| `LICENSE` | **新規** |
| `README.md` | **新規** |
| `.github/workflows/ci.yml` | **新規** |
| `.github/workflows/release.yml` | **新規** |
