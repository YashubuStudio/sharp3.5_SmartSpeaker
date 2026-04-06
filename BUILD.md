# Smart Speaker ビルドガイド

ソースコードからビルドする際の手順と注意事項をまとめています。

## 1. 必須ツール

| ツール | 用途 | インストール方法 |
|--------|------|-----------------|
| **Rust** (1.70+) | コンパイラ | `winget install Rustlang.Rustup` |
| **LLVM** | `whisper-rs-sys` の bindgen が `libclang.dll` を使用 | `winget install LLVM.LLVM` |
| **CMake** | `whisper-rs-sys` が whisper.cpp をコンパイルする際に使用 | `winget install Kitware.CMake` |
| **Visual Studio Build Tools** | MSVC リンカ・C++ コンパイラ | `winget install Microsoft.VisualStudio.2022.BuildTools` + C++ ワークロード |

> LLVM と CMake をインストール後、**新しいターミナルを開いて** PATH を反映させてください。

### 環境変数

LLVM が PATH に入っていない場合は `LIBCLANG_PATH` を設定する必要があります:

```powershell
# PowerShell（一時的に設定）
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"

# 永続的に設定する場合
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", "C:\Program Files\LLVM\bin", "User")
```

## 2. CPU 版ビルド

NVIDIA GPU がない、または CUDA を使わない場合のビルド方法です。

```powershell
# デバッグビルド
cargo build

# リリースビルド（推奨）
cargo build --release

# 実行
cargo run --release
```

## 3. CUDA 版ビルド（GPU 高速推論）

NVIDIA GPU で Whisper 推論を高速化する場合に使用します。

### 追加の前提条件

| ツール | バージョン | 備考 |
|--------|-----------|------|
| **NVIDIA GPU** | GTX 1060 以上 | CUDA 対応 GPU |
| **NVIDIA ドライバー** | 570.x 以上 | `nvidia-smi` で確認 |
| **CUDA Toolkit** | 12.4 / 12.6 / 12.8 | 13.x は非対応 |

### CUDA Toolkit インストール

1. [CUDA Toolkit](https://developer.nvidia.com/cuda-toolkit) からダウンロード
2. Windows / x86_64 / exe (local) を選択
3. **CUDA Toolkit 12.8** を選択（12.4、12.6 でも動作可）
4. Express インストールを実行
5. PC を再起動

### 確認

```powershell
nvcc --version
# cuda_12.8.x が表示されればOK

nvidia-smi
# GPU名とドライバーバージョンが表示されればOK
```

### ビルド

```powershell
cargo build --release --features cuda
```

## 4. ビルド時の注意事項

### whisper-rs-sys のコンパイルに時間がかかる

`whisper-rs-sys` は whisper.cpp の C++ コードをソースからコンパイルするため、初回ビルドに **数分〜10分程度** かかります。ハングではなく正常な動作です。

### `libclang` が見つからないエラー

```
Unable to find libclang: "couldn't find any valid shared libraries matching:
['clang.dll', 'libclang.dll']"
```

LLVM がインストールされていないか、PATH が通っていません:

```powershell
winget install LLVM.LLVM
# またはインストール済みなら環境変数を設定
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```

### CMake が見つからないエラー

```
Failed to find CMake
```

```powershell
winget install Kitware.CMake
```

### CUDA 版で `NotPresent` エラー

```
called `Result::unwrap()` on an `Err` value: NotPresent
```

CUDA Toolkit が未インストール、または `CUDA_PATH` 環境変数が未設定です。CUDA Toolkit をインストールして PC を再起動してください。

### 構造体サイズのオーバーフローエラー (whisper-rs 0.15 以前)

```
error[E0080]: attempt to compute `1_usize - 296_usize`, which would overflow
```

`whisper-rs` 0.15 (`whisper-rs-sys` 0.14.1) は LLVM 22.x と非互換です。`whisper-rs` を 0.16 以上にアップデートしてください:

```toml
# Cargo.toml
whisper-rs = "0.16"
```

## 5. 依存サービスのセットアップ

ビルド後、実行には以下のサービスが必要です。`setup.bat` で自動セットアップできます。

| サービス | ポート | セットアップ |
|----------|--------|-------------|
| **Ollama** | localhost:11434 | `setup.bat` で自動インストール |
| **VOICEVOX** | localhost:50021 | [公式サイト](https://voicevox.hiroshiba.jp/)から手動インストール |

```powershell
# Ollama モデルのダウンロード
ollama pull gemma3:4b
ollama pull nomic-embed-text    # RAG を使う場合

# Whisper モデルは初回起動時に自動ダウンロード（約 1.5GB）
```

## 6. 開発コマンド

```powershell
# フォーマット
cargo fmt

# Lint
cargo clippy

# テスト
cargo test

# ドキュメント生成
cargo doc --open
```

## 7. Cargo features 一覧

| Feature | 説明 | デフォルト |
|---------|------|-----------|
| `cuda` | NVIDIA CUDA による Whisper GPU 推論 | off |

```powershell
# CPU版（デフォルト）
cargo build --release

# CUDA版
cargo build --release --features cuda
```
