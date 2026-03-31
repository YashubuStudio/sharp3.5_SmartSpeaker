# 配布設計書

## 概要

Smart Speaker（AIコンシェルジュ）をエンドユーザーに配布するための設計。
7zアーカイブを展開するだけで利用開始でき、同梱の `setup.bat` で依存コンポーネントを一括セットアップできる。

## ビルドバリアント

| バリアント | ビルドコマンド | CUDA依存 | 対象ユーザー |
|-----------|---------------|----------|-------------|
| CPU版（デフォルト配布） | `cargo build --release --no-default-features` | なし | 全ユーザー |
| GPU版（オプション） | `cargo build --release` | 利用者が別途CUDA Toolkitをインストールする必要がある | NVIDIA GPU搭載PC |

## 配布パッケージ構成

```
smart_speaker-v{VERSION}-windows-{cpu|cuda}.7z
  │
  └─ smart_speaker-v{VERSION}-windows-{cpu|cuda}/
      ├── smart_speaker.exe              # メインバイナリ
      ├── setup.bat                       # 依存コンポーネント一括セットアップ
      ├── run.bat                         # ランチャー（依存起動＋アプリ起動）
      ├── .variant                        # バリアント識別ファイル ("cpu" or "cuda")
      ├── wakeword.rpw                    # ウェイクワードモデル
      ├── config/
      │   └── settings.toml              # ユーザー設定
      ├── models/
      │   └── README.txt                 # モデルダウンロード案内
      ├── data/
      │   └── knowledge/                 # RAG用ナレッジ（空）
      ├── README.md
      └── LICENSE
```

### 同梱しないもの

| ファイル | 理由 | 対応方法 |
|---------|------|---------|
| Whisperモデル (ggml-*.bin) | 1〜3GBと巨大 | 初回起動時に対話的ダウンロード（実装済み） |
| Ollama | 独自のGPU対応・更新機構を持つ | `setup.bat` で自動インストール |
| VOICEVOX | 同上 | `setup.bat` でインストール案内 |
| CUDA DLL | ライセンス・サイズの問題 | 利用者が別途 CUDA Toolkit をインストール |

## 外部依存コンポーネント

| コンポーネント | 用途 | CPU版 | GPU版 | セットアップ方法 |
|---------------|------|:-----:|:-----:|----------------|
| NVIDIA GPU ドライバー | GPU動作 | 不要 | **必須** | [NVIDIA公式](https://www.nvidia.com/Download/index.aspx) |
| CUDA Toolkit 12.x | Whisper GPU推論 | 不要 | **必須** | `setup.bat` が案内 |
| Ollama | LLM推論 | **必須** | **必須** | `setup.bat` が自動インストール |
| VOICEVOX | 音声合成 | **必須** | **必須** | `setup.bat` がインストール案内 |
| Whisperモデル | 音声認識 | 自動DL | 自動DL | 初回起動時に自動ダウンロード |

## セットアップ手順

### CPU版

```
1. CPU版 .7z を任意の場所に展開（例: デスクトップ、ドキュメント）
2. setup.bat を実行（Ollama自動インストール + モデル取得 + VOICEVOX案内）
3. VOICEVOX をインストール・起動
4. run.bat を実行
```

### GPU版

```
1. GPU版 .7z を任意の場所に展開
2. setup.bat を実行
   - CUDA Toolkit 未検出時はインストール案内を表示
   - Ollama 自動インストール + モデル取得
   - VOICEVOX インストール案内
3. CUDA Toolkit 12.x をインストール（未インストールの場合）
4. VOICEVOX をインストール・起動
5. run.bat を実行
```

### setup.bat の処理フロー

```
[1/5] ディレクトリ作成 (data/, models/)
[2/5] CUDA チェック（GPU版のみ: nvcc --version → 案内 or スキップ）
[3/5] Ollama インストール（winget install Ollama.Ollama → フォールバック: URL案内）
[4/5] Ollama モデルDL（gemma3:4b, nomic-embed-text）
[5/5] VOICEVOX 確認（起動中 → OK / 未検出 → インストール案内）
```

### run.bat の処理フロー

```
[1/3] Ollama 起動確認（未起動なら ollama serve で起動）
[2/3] VOICEVOX 起動確認（未起動なら自動起動 or 手動案内）
[3/3] smart_speaker.exe 起動
```

### トラブルシューティング

| 症状 | 原因 | 対処 |
|------|------|------|
| `cudart64_12.dll not found` | CUDA Toolkit未インストール | CUDA Toolkit 12.x をインストール、PC再起動 |
| `nvidia-smi` でGPUが表示されない | ドライバー未インストール or GPU非搭載 | GPUドライバーをインストール、またはCPU版を使用 |
| Whisper推論が遅い（CPU並） | CUDAバージョン不一致 | `nvcc --version` でCUDA 12.xか確認 |
| OllamaがGPUを使わない | ドライバーが古い | 最新GPUドライバーに更新 |
| `store.jsonl` 書き込みエラー | 書き込み権限のない場所に展開 | ユーザーフォルダ配下に展開し直す |

## Whisperモデル選択ガイド

| モデル | サイズ | VRAM (GPU) | 精度 | 速度 | 推奨環境 |
|-------|--------|-----------|------|------|---------|
| ggml-tiny.bin | 75MB | ~1GB | 低 | 最速 | テスト用 |
| ggml-base.bin | 142MB | ~1GB | 中低 | 速い | 低スペックPC |
| ggml-small.bin | 466MB | ~2GB | 中 | 普通 | 一般的なPC |
| ggml-medium.bin | 1.5GB | ~5GB | 高 | やや遅 | ミドルスペック |
| ggml-large-v3-turbo.bin | 1.6GB | ~5GB | 最高 | 普通 | 推奨 |
| ggml-large-v3.bin | 3.1GB | ~10GB | 最高 | 遅い | ハイスペック |

設定ファイル `config/settings.toml` の `[stt] model_path` でいつでも切替可能。

## Whisperモデル自動ダウンロード機能

### 動作フロー

```
起動 → settings.toml の model_path を確認
  │
  ├─ モデルファイルが存在する → そのまま使用
  │
  └─ モデルファイルが存在しない → 対話的モデル選択
       │
       ├─ モデル一覧を表示（サイズ・説明付き）
       ├─ settings.toml の設定値をデフォルト選択としてマーク
       ├─ ユーザーが番号で選択（Enter でデフォルト）
       └─ HuggingFace から自動ダウンロード → models/ に保存
```

### 実装場所

- `src/stt/model_downloader.rs` — ダウンロードと対話的選択
- `src/main.rs` — 選択結果をSTT設定に反映

### 設計ポイント

- 設定ファイルと異なるモデルを選んでも動作する（実行時にパスを上書き）
- `.part` ファイルで途中ダウンロード対策（完了後にリネーム）
- 10MB単位で進捗表示
- 既知モデル名以外はエラー（任意URLからのDLは防止）

## CI/CD (GitHub Actions)

### ワークフロー

| ファイル | トリガー | 内容 |
|---------|---------|------|
| `.github/workflows/ci.yml` | push/PR to main | fmt, clippy, build, test (CPU版) |
| `.github/workflows/release.yml` | `v*` タグpush | CPU版/CUDA版ビルド → GitHub Release公開 |

### リリースフロー

```
git tag v0.1.0 && git push origin v0.1.0
  │
  ├─ build-cpu ジョブ
  │   ├─ cargo build --release --no-default-features
  │   ├─ .variant ファイル作成 ("cpu")
  │   └─ 7z アーカイブ作成 (smart_speaker-v0.1.0-windows-cpu.7z)
  │
  ├─ build-cuda ジョブ
  │   ├─ CUDA Toolkit 12.6 インストール
  │   ├─ cargo build --release --features cuda
  │   ├─ .variant ファイル作成 ("cuda")
  │   └─ 7z アーカイブ作成 (smart_speaker-v0.1.0-windows-cuda.7z)
  │
  └─ release ジョブ (両ビルド完了後)
      └─ GitHub Release 作成 + 全アーティファクト添付
```

### リリース成果物

| ファイル | 内容 | 対象ユーザー |
|---------|------|-------------|
| `smart_speaker-v*-windows-cpu.7z` | CPU版ポータブル版 | 全ユーザー（推奨） |
| `smart_speaker-v*-windows-cuda.7z` | GPU版ポータブル版 | NVIDIA GPU搭載PC |

### バージョンタグの付け方

```powershell
# 1. Cargo.toml のバージョンを更新
# 2. コミット
git add -A && git commit -m "chore: bump version to 0.2.0"
# 3. タグ付け & push
git tag v0.2.0
git push origin main --tags
```

## ローカルリリースビルド

`scripts/release.bat` でローカルでもリリースパッケージを作成可能。

```powershell
# デフォルト（CUDA版ビルド + パッケージング）
scripts\release.bat

# ビルドスキップ（既にビルド済み）
scripts\release.bat --no-build

# ナレッジファイル同梱
scripts\release.bat --with-knowledge
```

出力先: `release/smart_speaker-v{VERSION}/`
配布時はこのフォルダを `7z a` で圧縮する。

## 将来の拡張

1. **自動アップデート**: GitHub Releases APIで最新版チェック → ダウンロード
2. **GPU版セットアップ簡易化**: CUDA Toolkit インストール手順のガイド強化
3. **依存コンポーネント一括セットアップ**: Ollama/VOICEVOXのサイレントインストール
4. **WinGet対応**: `winget install smart-speaker` での配布
5. **ダウンロード再開**: 中断したダウンロードの途中再開（Range header）
