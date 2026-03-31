========================================
  Whisper モデル配置ディレクトリ
========================================

このディレクトリに Whisper の ggml 形式モデルを配置してください。

■ ダウンロード先
  https://huggingface.co/ggerganov/whisper.cpp/tree/main

■ モデル一覧
  ggml-tiny.bin          (75MB)   - テスト用
  ggml-base.bin          (142MB)  - 軽量・低スペックPC向け
  ggml-small.bin         (466MB)  - バランス型
  ggml-medium.bin        (1.5GB)  - 高精度
  ggml-large-v3-turbo.bin (1.6GB) - 推奨（高精度・高速）
  ggml-large-v3.bin      (3.1GB)  - 最高精度

■ 設定方法
  config\settings.toml の [stt] セクションで指定:

  [stt]
  model_path = "models/ggml-large-v3-turbo.bin"

========================================
