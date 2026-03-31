# ハードウェア要件

## 使用コンポーネント

| コンポーネント | モデル/設定 | 主な負荷 |
|---|---|---|
| Whisper STT | ggml-large-v3-turbo (CUDA) | GPU VRAM |
| Ollama LLM | gemma3:4b | GPU VRAM |
| Ollama Embedding | nomic-embed-text | GPU VRAM |
| VOICEVOX | ローカルサーバー | CPU/GPU |
| Rustpotter | sakura.rpw | CPU (軽量) |

## NVIDIAドライバー

CUDA版を使用する場合、NVIDIAドライバーのバージョンが重要です。

| CUDA版 | 最低ドライバーバージョン |
|---|---|
| CUDA 12.8 | **570.x 以上** |
| CUDA 12.6 | 560.x 以上 |
| CUDA 12.4 | 550.x 以上 |

### ドライバーの確認方法

```powershell
nvidia-smi
```

出力の右上に `Driver Version: XXX.XX` が表示されます。570未満の場合はアップデートが必要です。

### ドライバーの更新方法

1. [NVIDIA Drivers](https://www.nvidia.com/drivers) にアクセス
2. GPUモデルを選択して最新の Game Ready または Studio ドライバーをダウンロード
3. インストール後、PCを再起動

> **注意**: リリースバイナリ（7z/ZIP）を別のPCで実行する場合、そのPCのNVIDIAドライバーが古いとCUDA関連のエラーで起動に失敗します。必ず事前にドライバーバージョンを確認してください。

## GPU (VRAM)

### モデル別VRAM使用量

| モデル | VRAM (概算) |
|---|---|
| Whisper large-v3-turbo (CUDA) | ~3 GB |
| Whisper medium | ~1.5 GB |
| Whisper small | ~0.5 GB |
| gemma3:4b (Q4量子化) | ~3 GB |
| nomic-embed-text | ~0.3 GB |
| VOICEVOX (GPU mode) | ~1-2 GB |

### 構成パターン

| 構成 | Whisper | LLM | 合計VRAM | 対象GPU |
|---|---|---|---|---|
| フル性能 | large-v3-turbo (3GB) | gemma3:4b (3GB) | ~6.3 GB | RTX 3060 12GB 以上 |
| ラップトップ向け | medium (1.5GB) | gemma3:4b (3GB) | ~4.8 GB | RTX 3060 Laptop 6GB |
| 軽量 | small (0.5GB) | gemma3:4b (3GB) | ~3.8 GB | RTX 3060 Laptop 6GB |

※ 合計にはnomic-embed-text (~0.3GB) を含む。OllamaはアイドルモデルをVRAMからアンロードするため、ピーク時と通常時で差がある。

### 推奨GPU

| レベル | GPU例 | VRAM | 備考 |
|---|---|---|---|
| 最低限 (ラップトップ) | RTX 3060 Laptop | 6 GB | Whisper medium以下が必要 |
| 最低限 (デスクトップ) | RTX 3060 | 12 GB | 全モデル同時ロード可能 |
| 推奨 | RTX 4060 Ti | 16 GB | 余裕あり |
| 快適 | RTX 4070 以上 | 12+ GB | 推論速度も高速 |

## CPU

| 要件 | 詳細 |
|---|---|
| 最低限 | 4コア / 8スレッド (Ryzen 5 / Core i5) |
| 推奨 | 6コア以上 |

Whisper/LLMはGPUオフロードするため、CPUの負荷は比較的低い。主な用途はRustpotter (常時)、オーディオI/O、パイプライン制御。

## RAM

| 要件 | 容量 |
|---|---|
| 最低限 | 16 GB |
| 推奨 | 32 GB |

内訳: OS ~4GB + VOICEVOX ~2GB + Ollama管理 ~2-4GB + Whisperバッファ ~1GB + アプリ本体 ~0.5GB

## ストレージ

| 項目 | サイズ |
|---|---|
| Whisper large-v3-turbo モデル | ~1.5 GB |
| Whisper medium モデル | ~1.5 GB |
| Whisper small モデル | ~466 MB |
| gemma3:4b | ~2.5 GB |
| nomic-embed-text | ~0.3 GB |
| VOICEVOX | ~1-2 GB |
| CUDA Toolkit | ~3-5 GB |
| Rustツールチェーン + ビルド | ~2-3 GB |
| **合計 (目安)** | **~10-15 GB** |

SSD推奨（モデルロード時間に大きく影響）。

## 推奨構成まとめ

### デスクトップ

| 項目 | 最低限 | 推奨 |
|---|---|---|
| GPU | RTX 3060 12GB | RTX 4060 Ti 16GB |
| CPU | 4C/8T (i5 / Ryzen 5) | 6C/12T 以上 |
| RAM | 16 GB | 32 GB |
| ストレージ | SSD 20GB 空き | SSD 50GB 空き |
| OS | Windows 10/11 | Windows 11 |
| NVIDIAドライバー | 570.x以上 | 最新版 |

### ラップトップ

| 項目 | 最低限 | 推奨 |
|---|---|---|
| GPU | RTX 3060 Laptop 6GB | RTX 4060 Laptop 8GB |
| CPU | 4C/8T | 6C/12T 以上 |
| RAM | 16 GB | 32 GB |
| ストレージ | SSD 20GB 空き | SSD 50GB 空き |
| Whisperモデル | small | medium |
| NVIDIAドライバー | 570.x以上 | 最新版 |

## LLMモデル変更時の影響

| モデル | 追加VRAM | 必要GPU |
|---|---|---|
| gemma3:4b (デフォルト) | ~3 GB | RTX 3060 Laptop 6GB~ |
| gemma3:12b | ~7 GB | RTX 4070 12GB~ |
| gemma3:27b | ~16 GB | RTX 4090 24GB |

## Whisperモデルのダウンロード

```bash
# large-v3-turbo (最高精度)
curl -L -o models/ggml-large-v3-turbo.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin

# medium (ラップトップ推奨)
curl -L -o models/ggml-medium.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin

# small (軽量)
curl -L -o models/ggml-small.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin
```

`config/settings.toml` でモデルパスを変更:

```toml
[stt]
model_path = "models/ggml-medium.bin"  # 環境に合わせて変更
```
