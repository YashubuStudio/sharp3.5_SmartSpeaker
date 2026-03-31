# Claude Code × Remotion プロンプトガイド
## EP: AIスマートスピーカー セットアップガイド

---

## 全体戦略

Claude Codeは「1回のプロンプトで完璧なもの」を求めるより、**段階的に構築→確認→修正**のサイクルを回すほうが圧倒的に品質が上がります。以下の順番で進めてください。

```
Phase 0: 音声パイプライン（台本・音声配置 → Whisper alignment → フレーム数算出）
Phase 1: プロジェクト基盤（config + 共通部品）
Phase 2: 各シーンを1つずつ作成・プレビュー確認
Phase 3: シーン結合・タイミング調整
Phase 4: 仕上げ（エフェクト・微調整）
```

**重要**: Phase 0が全ての基盤です。ユーザーが配置した音声ファイルと
Whisperのタイムスタンプからシーンごとの正確なフレーム数が決まります。
Phase 2以降のフレーム数はすべて`scene-frames.json`から取得してください。

**前提**: 各シーンの台本(.md)と音声(.wav)は`data/input/{NN}-{scene-name}/`に
ユーザーが手動で配置済みです。

**映像ルール**:
- 背景は常に`public/images/background.png`を使用。背景色の生成は禁止。
- 画面上に字幕は表示しない。ナレーションは音声のみ。
- 全てのビジュアル要素は画面幅の85%以上を使うこと。
- コードブロック・ターミナルの背景は半透明にして背景画像を透かす。
- 全てのPhaseに具体的なアニメーション指示を含めること（静的表示のみは禁止）。
- 各コンテンツタイプのAnimation Recipeに従ったアニメーション構成にすること。
- アニメーションにはRemotionのspring()やinterpolate()を明示的に指定する。
- アニメーションはsrc/utils/animations.tsのユーティリティパターンを活用する。
- 3つ以上のPhaseがあるシーンには🎥カメラワーク指示（focusZoom/pullBack/pan/rackFocus）を含める。
- カメラはコンテンツラッパーに適用。背景画像は常に静止。
- 値の変化、コード修正、状態変化には⚡遷移指示（countUp/code fix morph/panel transform/status change）を使う。

---

## シーン構成

| # | sceneId | シーン名 | 内容 | 主なビジュアル |
|---|---------|----------|------|---------------|
| 01 | 01-opening | OpeningScene | 自己紹介・アプリ概要 | タイトルカード、アプリ概念図 |
| 02 | 02-overview | OverviewScene | しくみの概要 | パイプライン図、サービステーブル |
| 03 | 03-version-select | VersionSelectScene | バージョン選び | 3カード比較、判断フロー |
| 04 | 04-extract | ExtractScene | 7z展開 | フォルダツリー図 |
| 05 | 05-setup | SetupScene | setup.bat実行 | ターミナル出力、プログレスバー |
| 06 | 06-voicevox | VoicevoxScene | VOICEVOXインストール | ブラウザ風UI、ステップリスト |
| 07 | 07-run | RunScene | run.bat起動・使い方 | ターミナル出力、使い方ステップ |
| 08 | 08-wakeword | WakewordScene | ウェイクワードカスタマイズ | ブラウザUI、設定ファイル表示 |
| 09 | 09-troubleshoot | TroubleshootScene | トラブルシューティング | FAQ形式カード |
| 10 | 10-summary | SummaryScene | まとめ | ステップチェックリスト |

---

## Phase 0: 音声パイプライン

### Prompt 0-1: 入力ファイル配置ガイド

以下の構造で`data/input/`にファイルを配置してください：

```
data/input/
├── 01-opening/
│   ├── script.md
│   └── audio.wav
├── 02-overview/
│   ├── script.md
│   └── audio.wav
├── 03-version-select/
│   ├── script.md
│   └── audio.wav
├── 04-extract/
│   ├── script.md
│   └── audio.wav
├── 05-setup/
│   ├── script.md
│   └── audio.wav
├── 06-voicevox/
│   ├── script.md
│   └── audio.wav
├── 07-run/
│   ├── script.md
│   └── audio.wav
├── 08-wakeword/
│   ├── script.md
│   └── audio.wav
├── 09-troubleshoot/
│   ├── script.md
│   └── audio.wav
└── 10-summary/
    ├── script.md
    └── audio.wav
```

各scene scriptの内容は台本の各セクションに対応。
台本中の ```` ``` ```` で囲まれたコードブロック（フォルダ構成、ターミナル出力、toml設定）は **visual-only** として扱う（音声には含まれない）。

### Prompt 0-2: スクリプトパーサー

```
scripts/parse-scripts.ts を作成して。

data/input/ 以下の全ディレクトリを番号順に読み込み、
各 script.md を以下のルールでパースする：

1. ## 見出し → セクションタイトルセグメント
2. ``` ``` の外のテキスト → type: "narration" セグメント（Whisper照合対象）
3. ``` ``` の中のテキスト → type: "visual-only" セグメント（画面表示のみ、音声なし）
4. <!-- visual: xxx --> コメント → 次のセグメントの VisualDirective

出力: data/script-structured.json（VideoProps形式、タイムスタンプは未設定）

実行: npx ts-node scripts/parse-scripts.ts data/input/ data/script-structured.json
```

### Prompt 0-3: 音声連結

```
scripts/concat-audio.ts を作成して。

data/input/ 以下の全ディレクトリを番号順に読み込み、
各 audio.wav を0.5秒の無音を挟んで連結する。

出力:
- public/audio/full.wav（連結済み音声）
- data/segment-boundaries.json（各シーンのstartSec/endSec）

ffmpegを使用。

実行: npx ts-node scripts/concat-audio.ts data/input/ public/audio/full.wav data/segment-boundaries.json
```

### Prompt 0-4: Whisperアライメント

```
scripts/align-timestamps.py を作成して。

public/audio/full.wav と data/script-structured.json を入力として、
faster-whisperでword-level timestampを取得し、
narrationセグメントのみに照合する（visual-onlyセグメントはスキップ）。

visual-onlyセグメントのタイミング:
  startSec = 直前のnarrationのendSec
  endSec = 直後のnarrationのstartSec

出力: data/script-aligned.json（完全なVideoProps形式）

実行: python scripts/align-timestamps.py public/audio/full.wav data/script-structured.json data/script-aligned.json
```

### Prompt 0-5: フレーム数算出

```
scripts/compute-frames.ts を作成して。

data/script-aligned.json を読み込み、fps=30で全シーン・全セグメントの
startFrame/endFrame/durationFrames を計算する。

出力: data/scene-frames.json

実行: npx ts-node scripts/compute-frames.ts data/script-aligned.json data/scene-frames.json
```

---

## Phase 1: プロジェクト基盤

### Prompt 1-1: プロジェクト初期化

```
Remotionプロジェクトを初期化して。

npx create-video@latest で作成し、以下を設定:
- FPS: 30
- 解像度: 1920x1080
- src/Root.tsx: calculateMetadata で data/script-aligned.json の
  totalDurationSec から durationInFrames を動的に算出
- src/types.ts: VideoProps, SceneData, ScriptSegment, WordTimestamp,
  VisualDirective の型定義を作成

package.json に以下を追加:
- @remotion/media-utils
- ts-node (devDependency)
```

### Prompt 1-2: 設定ファイル

```
src/config.ts を作成して。

以下の設定を定義:

カラーテーマ（ダーク系）:
  accent: '#58a6ff' (cyber blue)
  success: '#3fb950' (green)
  error: '#f85149' (red)
  warning: '#d29922' (orange)
  codeBg: 'rgba(30, 34, 40, 0.85)' (半透明ダークグレー)
  terminalBg: 'rgba(13, 17, 23, 0.9)' (半透明ダーク)
  textPrimary: '#e6edf3' (white)
  textSecondary: '#8b949e' (gray)

シーン名マッピング（scene-frames.jsonと対応）:
  '01-opening': 'OpeningScene'
  '02-overview': 'OverviewScene'
  '03-version-select': 'VersionSelectScene'
  '04-extract': 'ExtractScene'
  '05-setup': 'SetupScene'
  '06-voicevox': 'VoicevoxScene'
  '07-run': 'RunScene'
  '08-wakeword': 'WakewordScene'
  '09-troubleshoot': 'TroubleshootScene'
  '10-summary': 'SummaryScene'

アニメーションデフォルト:
  springConfig: { damping: 12, mass: 0.5, stiffness: 100 }
  defaultStaggerDelay: 8 (frames)
  defaultFadeFrames: 10
```

### Prompt 1-3: 共通コンポーネント + アニメーションユーティリティ

```
以下のファイルを作成して:

■ src/utils/animations.ts
以下のアニメーションユーティリティ関数を全て実装:

1. typewriter(frame, text, startFrame, charsPerFrame=0.5) → string
   テキストを1文字ずつ表示

2. visibleLines(frame, totalLines, startFrame, framesPerLine=4) → number
   コードの行数を段階的に増やす

3. lineOpacity(frame, lineIndex, startFrame, framesPerLine=4) → number
   特定行のopacity（0 or 1）

4. highlightSlideIn(frame, startFrame, durationFrames=10) → { widthPercent, opacity }
   ハイライトバーの左からスライドイン

5. staggeredEntrance(frame, index, startFrame, staggerDelay=8, config?) → { opacity, scale, translateY }
   リスト要素の順次登場

6. drawEdge(frame, startFrame, durationFrames, pathLength) → strokeDashoffset
   SVGパスの描画アニメーション

7. pulse(frame, startFrame, pulseCount=3, framesPerPulse=15) → scale
   パルス強調

8. countUp(frame, startFrame, durationFrames, fromValue, toValue, decimals=4) → string
   数値カウントアップ

9. focusZoom(frame, startFrame, durationFrames, targetY, zoomScale=1.5) → { scale, translateY }
   カメラズーム

10. slideIn(frame, startFrame, durationFrames=12, direction, distance=200) → { translateX, translateY, opacity }
    スライドイン

11. colorMorph(frame, startFrame, durationFrames, fromColor, toColor) → string
    カラー遷移

■ src/components/SceneBackground.tsx
- public/images/background.png を <Img src={staticFile('images/background.png')} /> で全画面表示
- 全シーンの基底レイヤー

■ src/components/SectionTitle.tsx
- タイトルテキスト（48-64px、bold、center）
- spring()でscale 0.5→1.0のポップアップ登場
- 下にaccent色の水平ラインが中央から左右に伸びるアニメーション
- props: title, subtitle?, startFrame

■ src/components/ProgressBar.tsx
- 画面下部に細いバー（高さ4px、accent色）
- interpolate()で左から右へ進行
- props: progress (0-1)

■ src/components/CodeBlock.tsx
- 半透明ダーク背景（codeBg）、角丸、横幅90%
- 行番号 + シンタックスハイライト風の色分け
- visibleLines/lineOpacityパターンで行ごと表示
- highlightSlideInパターンでハイライト行の強調
- props: code, language?, highlightLines?, startFrame, framesPerLine?

■ src/components/TerminalOutput.tsx
- ターミナル風ウィンドウ（terminalBg、タイトルバーに赤黄緑ドット）
- 横幅90%、font-size 26px
- typewriterパターンでコマンド入力表示
- 出力行はstaggeredEntranceで順次表示
- props: lines, startFrame

■ src/components/StepList.tsx
- 番号付きステップリスト
- 各ステップがstaggeredEntranceで順次登場
- アクティブステップにaccent色ハイライト
- 完了ステップにチェックマーク（spring()でポップアップ）
- props: steps, activeIndex, startFrame

■ src/components/ComparisonCards.tsx
- 横並び（または縦並び）のカード比較
- 各カードがstaggeredEntranceで登場
- ハイライトカードにaccent色ボーダー + scale(1.02)
- props: cards, highlightIndex?, startFrame

■ src/hooks/useCurrentSegment.ts
- useCurrentFrame()を使い、script-aligned.jsonのセグメントから
  現在フレームに対応するセグメントを返すフック
```

---

## Phase 2: 各シーン作成

> **重要**: 全てのフレーム数は`scene-frames.json`の値を使うこと。
> 台本内のコードブロック（``` で囲まれた部分）は音声にはなりません。
> これらはvisual-onlyセグメントとして画面表示のみに使います。
> 画面上に字幕は表示しない。ナレーションは音声のみ。
> **全ての■Phaseに具体的なアニメーション指示を含めること（静的表示のみは禁止）。**
> **全てのビジュアル要素にサイズ（width %, font-size px）を明記すること。**

### Prompt 2-1: OpeningScene（01-opening）

```
src/scenes/OpeningScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "01-opening" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「こんにちは、AIコンシェルジュを作ろうです。
今回は、私が作っているAIスマートスピーカーを皆さんのPCで動かす方法を説明します。
これは何かというと、PCのマイクに向かって話しかけると、AIが考えて音声で返事してくれるアプリです。
ウェイクワードを言うと起動して、質問すればAIが日本語で答えてくれます。
ウェイクワードは自分の好きな言葉に変えられるので、その方法も後で説明します。
処理は全部あなたのPC上で動くので、クラウドに音声データが送られることはありません。
配布した7zファイルを展開して、batファイルを2つ叩くだけで使えるようになります。
順番に説明していきます。」

ビジュアル構成:

■ Phase 1（タイトル登場、最初の2文の区間）:
SectionTitleコンポーネントで動画タイトル
「AIスマートスピーカー セットアップガイド」を表示。
font-size 56px、bold、画面中央。
spring({ damping: 12 })でscale(0.5→1.0) + opacity(0→1)で登場。
タイトル下にaccent色の水平ライン（幅0%→60%）が中央から左右に0.5秒で伸びる。
さらにその下に「AIコンシェルジュを作ろう」のチャンネル名が
font-size 28px、textSecondary色でフェードイン（0.3秒遅延）。

■ Phase 2（アプリ概念図、「PCのマイクに〜」から「音声データが送られることはありません」の区間）:
タイトルがscale(1.0→0.8) + translateY(0→-200px)で画面上部へ移動（0.5秒）。
画面中央にアプリの概念図がspring()で登場:
  左に人のアイコン（silhouette、80px）がslideInで左から登場
  中央にPCアイコン（120px）がscale(0→1)でポップアップ
  右にスピーカーアイコン（80px）がslideInで右から登場
アイコン間に矢印がdrawEdgeパターンで描画（0.3秒ずつ）。
人→PC の矢印の上に「話しかける」ラベル（font-size 24px）がフェードイン。
PC→スピーカーの矢印の上に「AIが返事」ラベルがフェードイン。
PCアイコンの下に「全てローカル処理」テキスト（font-size 20px、success色）が
pulse(2回)で強調しながら登場。

■ Phase 3（導入まとめ、「batファイルを2つ叩くだけ」の区間）:
概念図がopacity(1.0→0.3)でフェードダウン。
画面中央に大きなテキスト「batファイルを2つ叩くだけ」（font-size 48px、accent色）が
spring()でポップアップ登場。
テキスト下に2つのアイコン:
  📁 setup.bat → 🚀 run.bat
がstaggeredEntrance(staggerDelay=10)で左から順に登場。
各アイコンにラベル「初回のみ」「毎回」がfont-size 20px、textSecondaryでフェードイン。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
```

### Prompt 2-2: OverviewScene（02-overview）

```
src/scenes/OverviewScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "02-overview" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「まず簡単にしくみを説明します。
裏ではこの3つのサービスが動いています：
Ollama、AIの頭脳。質問を理解して返答を考える。
VOICEVOX、声。AIの返答を日本語の音声に変換する。
Whisper、耳。あなたの声をテキストに変換する。
セットアップではこれらをインストールしていきますが、ほぼ自動なので安心してください。」

visual-onlyセグメント（パイプライン図）:
マイク → ウェイクワード検出 → 音声録音 → 音声認識 → AI応答生成 → 音声合成 → スピーカー

ビジュアル構成:

■ Phase 1（パイプライン図、「しくみを説明します」の区間）:
SectionTitle「しくみの概要」がspring()でポップアップ（font-size 48px）→
0.5秒ホールド後にscale(1.0→0.7) + translateY(0→-280px)で上部へ退避。

パイプライン図 — Diagram/Pipeline Recipeに従い:
7つのノード（マイク、ウェイクワード検出、音声録音、音声認識、AI応答生成、音声合成、スピーカー）を
横一列に配置（画面幅90%、各ノード横幅12%）。
ノードがstaggeredEntrance(staggerDelay=6)で左から順にspring()ポップアップ。
各ノードはrounded rect、ダーク半透明背景、font-size 18px、textPrimary色。
ノード間の矢印がdrawEdgeパターンで前のノード登場完了直後に描画（0.3秒）。

🎥 カメラ: パイプライン全体が登場後、translateXパンで左端→右端を
ナレーション進行に合わせてゆっくり移動（全体を画面幅120%で描画し、-10%パン）。

■ Phase 2（サービステーブル、「3つのサービスが動いています」以降の区間）:
パイプライン図がscale(1.0→0.6) + translateY(0→-200px)で上部に縮小移動（0.5秒）。
パイプライン内の該当ノードにglow(accent色)がフェードイン。

画面下部60%に3つのサービスカードがstaggeredEntrance(staggerDelay=12)で登場:

カード1「Ollama」:
  🧠アイコン + "Ollama" (font-size 32px, bold) + "AIの頭脳" (font-size 24px, accent色)
  + "質問を理解して返答を考える" (font-size 20px, textSecondary)
  カード背景: codeBg、横幅28%、padding 20px
  登場時spring()でscale(0→1) + slideIn(bottom, 50px)

カード2「VOICEVOX」:
  🗣️アイコン + "VOICEVOX" + "声" + 説明テキスト
  同じスタイル、0.4秒遅延で登場

カード3「Whisper」:
  👂アイコン + "Whisper" + "耳" + 説明テキスト
  同じスタイル、0.8秒遅延で登場

各カードが登場するタイミングで、上のパイプライン図の対応ノードに
accent色のglowが0.3秒でフェードイン→前のノードのglowはフェードアウト。
→ 🎥 rackFocus風の演出。

■ Phase 3（安心メッセージ、最後の1文の区間）:
3カード全てにsuccess色の薄いボーダーが同時にフェードイン。
カード群の下に「ほぼ自動でインストール ✓」テキスト（font-size 28px、success色）が
spring()でポップアップ + pulse(2回)で強調。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
コードブロック・ターミナルの背景は半透明にして背景画像を透かす。
```

### Prompt 2-3: VersionSelectScene（03-version-select）

```
src/scenes/VersionSelectScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "03-version-select" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「渡した7zファイルは3種類あります。自分のPCに合ったものを選んでください。
CPU版。NVIDIA GPUがない人はこれ。どのWindows PCでも動く。音声認識が遅い。
CUDA-bundled版。NVIDIA GPU搭載ならこれがおすすめ。
GPU用のファイルが全部入っているのでそのまま動く。音声認識が高速。
CUDA版。NVIDIA GPU搭載プラスCUDA Toolkitを自分でインストールしたい人向け。
上級者向けなので、迷ったらCUDA-bundled版を使ってください。
判断基準はシンプルです：NVIDIA GPU持ってる→CUDA-bundled版、持ってない→CPU版。」

ビジュアル構成:

■ Phase 1（3カード登場、冒頭〜「選んでください」の区間）:
SectionTitle「どのバージョンを使う？」（font-size 48px）がspring()でポップアップ。
0.5秒後にtranslateY(0→-250px)で上部へ移動。

ComparisonCardsコンポーネントで3枚のカードを横並びに配置（各幅30%、gap 2%）:
カードがstaggeredEntrance(staggerDelay=10)で左から順に登場。
各カードはcodeBg背景、rounded rect、padding 24px。

■ Phase 2（CPU版説明、「CPU版」の区間）:
左のカード（CPU版）にaccent色ボーダーがフェードイン + scale(1.0→1.03)で拡大。
他2カードはopacity(1.0→0.5)にフェードダウン。→ rackFocus演出。

カード内容がstaggeredEntranceで上から順に表示:
  "CPU版" (font-size 28px, bold)
  "smart_speaker-v0.1.0-cpu.7z" (font-size 16px, textSecondary, monospace)
  "〜2MB" (font-size 20px)
  特徴リスト:
    "✓ どのPCでも動く" (success色)
    "△ 音声認識が遅い" (warning色)

■ Phase 3（CUDA-bundled版説明、「CUDA-bundled版」の区間）:
⚡ rackFocus遷移: CPU版カード → CUDA-bundled版カード
  CPU版: opacity 1.0→0.5, scale 1.03→1.0 (0.3秒)
  CUDA-bundled版: opacity 0.5→1.0, scale 1.0→1.03 (0.3秒)

中央カード内容が同様にstaggeredEntranceで表示:
  "CUDA-bundled版" (font-size 28px, bold)
  "smart_speaker-v0.1.0-cuda-bundled.7z" (font-size 16px)
  "〜480MB" (font-size 20px)
  特徴リスト:
    "✓ GPU搭載ならおすすめ" (success色)
    "✓ 音声認識が高速" (success色)
    "★ おすすめ" バッジがspring()でポップアップ（accent背景、白文字、font-size 18px）

■ Phase 4（CUDA版説明、「CUDA版」の区間）:
⚡ rackFocus遷移: CUDA-bundled版→CUDA版（同パターン）

右カード内容:
  "CUDA版" (font-size 28px, bold)
  "smart_speaker-v0.1.0-cuda.7z" (font-size 16px)
  "〜60MB" (font-size 20px)
  特徴リスト:
    "上級者向け" (textSecondary色)

■ Phase 5（判断フロー、「判断基準はシンプル」の区間）:
3カードがscale(1.0→0.7) + translateY(0→-180px)で上部に縮小移動（0.5秒）。
全カードopacity 0.6に統一。

画面下部50%にフローチャートが登場:
  「NVIDIA GPU持ってる？」(font-size 32px) がspring()でポップアップ
  → 左に「No」矢印がdrawEdgeで描画 → 「CPU版」ノード(error色ボーダー)がspring()登場
  → 右に「Yes」矢印がdrawEdge → 「CUDA-bundled版」ノード(success色ボーダー)がspring()登場
  「CUDA-bundled版」ノードにpulse(2回)の強調。

🎥 カメラ: Phase 2-4で各カード説明時にtranslateXで該当カード方向に微パン（±3%）。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
```

### Prompt 2-4: ExtractScene（04-extract）

```
src/scenes/ExtractScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "04-extract" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「まず、受け取った7zファイルを展開します。
7-Zipがインストールされていれば右クリック→展開でOKです。
持っていなければ7-zip.orgから無料でインストールできます。
展開先はデスクトップでもDドライブでも、どこでも大丈夫です。
展開するとこんなフォルダ構成になっています。」

visual-onlyセグメント（フォルダツリー）:
smart_speaker-v0.1.0-cuda-bundled/
├── smart_speaker.exe    ← 本体
├── setup.bat            ← 最初に1回だけ実行
├── run.bat              ← 毎回これで起動
├── config/
│   └── settings.toml    ← 設定ファイル（後で調整可能）
├── models/              ← Whisperモデルの保存先
├── data/
│   └── knowledge/       ← AIに覚えさせたい情報を入れる場所
└── wakeword.rpw         ← ウェイクワードの認識モデル

ビジュアル構成:

■ Phase 1（7z展開の説明、最初の4文の区間）:
SectionTitle「手順1: 7zファイルを展開」（font-size 48px）がspring()で登場→上部へ退避。

画面中央に7zファイルアイコン（大きなアーカイブ風の四角、120px、accent色ボーダー）が
spring()でscale(0→1)ポップアップ。
アイコン内に「.7z」テキスト（font-size 36px、bold）。

アイコンの右側に「右クリック → 展開」のテキストがtypewriterパターンで表示
（font-size 28px）。

0.5秒後、7zアイコンが「開く」アニメーション:
  アイコンの上辺がtranslateY(-20px)で開き、
  中からフォルダアイコンがspring()で飛び出す演出。

■ Phase 2（フォルダツリー表示、「こんなフォルダ構成」の区間）:
7zアイコンがopacity(1→0)でフェードアウト。

CodeBlockコンポーネントでフォルダツリーを表示:
  横幅90%、font-size 24px、monospace、codeBg背景。
  visibleLinesパターンで1行ずつ表示（framesPerLine=3）。

  重要ファイルにハイライト:
    setup.bat の行 → accent色のhighlightSlideIn
    run.bat の行 → accent色のhighlightSlideIn（0.3秒遅延）
  ハイライト時にscale(1.02)で行が微拡大。

  各行の「←」以降の説明テキストはtextSecondary色で表示。

🎥 カメラ: フォルダツリーが全行表示された後、
setup.batとrun.batの行にfocusZoom（scale 1.3、0.5秒）でフォーカス。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
コードブロック・ターミナルの背景は半透明にして背景画像を透かす。
```

### Prompt 2-5: SetupScene（05-setup）

```
src/scenes/SetupScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "05-setup" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「展開したフォルダの中にあるsetup.batをダブルクリックしてください。
これが自動的に必要なものをインストールしていきます。
[1/6]フォルダ作成。一瞬で終わります。
[2/6]GPU確認。CUDA-bundled版なら必要なファイルが同梱済みなので問題なく通ります。
[3/6]Ollamaのインストール。AIの頭脳にあたるサービスです。
入っていなければインストーラーを自動ダウンロードして実行します。
Ollamaのインストール画面が出たら、指示に従って進めてください。
[4/6]AIモデルのダウンロード。
gemma3:4b、AIの応答用、約2.5GB。
nomic-embed-text、知識検索用。
ネット回線によりますが、数分かかります。
[5/6]Whisperモデルのダウンロード。
音声認識用のモデルファイルです。約1.6GB。自動でダウンロードされます。
[6/6]VOICEVOXの確認。
ここでVOICEVOX not foundと表示されたら、次の手順で手動インストールします。
全体で10〜20分くらいかかります。ほとんどはダウンロードの待ち時間です。」

ビジュアル構成:

■ Phase 1（setup.bat説明、最初の2文の区間）:
SectionTitle「手順2: setup.batを実行」（font-size 48px）がspring()で登場→上部へ退避。

TerminalOutputコンポーネントでsetup.bat起動風の表示:
  ターミナルウィンドウ枠がscale(0.8→1.0)でポップアップ（横幅90%）。
  タイトルバーに「Smart Speaker Setup」テキスト。
  プロンプト「>」+ 「setup.bat」がtypewriterパターンで入力表示（30ms/文字）。
  カーソルが点滅。

■ Phase 2（ステップ1-2、フォルダ作成・GPU確認の区間）:
ターミナルがscale(1.0→0.5) + translateX(0→-35%)で左に縮小移動。

画面右60%にStepListコンポーネントが登場:
  6ステップのリスト（各ステップにアイコン + テキスト）
  ステップ1「フォルダ作成」がstaggeredEntranceで登場 →
    即座にチェックマーク✓がspring()でポップアップ（success色）
  ステップ2「GPU確認」が登場 →
    0.5秒後にチェックマーク✓がspring()でポップアップ

左のターミナルにも対応する出力行がtypewriterで同期表示:
  "[1/6] Creating directories..."
  "      Done"
  "[2/6] CUDA check..."

■ Phase 3（ステップ3、Ollamaインストールの区間）:
ステップ3「Ollamaインストール」がアクティブ化（accent色ハイライト + scale 1.02）。
左のターミナルに進捗表示:
  "[3/6] Checking Ollama..."
  "      Downloading Ollama installer..."
  typewriterパターンで表示。
  プログレスバー風の表示「[████████░░] 80%」がcountUpパターンで0→100%に進む。
  完了時に「Ollama installed successfully」がsuccess色でフェードイン。
ステップ3にチェックマーク✓。

🎥 カメラ: ステップ3の説明中にターミナル側にtranslateX微パン（-3%）で
ターミナル出力にフォーカス。

■ Phase 4（ステップ4-5、モデルダウンロードの区間）:
ステップ4「AIモデルダウンロード」がアクティブ化。
左のターミナルに:
  "[4/6] Downloading Ollama models..."
  "      gemma3:4b (2.5GB)" + プログレスバーアニメーション
  "      nomic-embed-text" + プログレスバーアニメーション
ステップ4にチェックマーク✓。

ステップ5「Whisperモデル」がアクティブ化。
  "[5/6] Checking Whisper model..."
  "      ggml-large-v3-turbo.bin (1.6GB)" + プログレスバーアニメーション
ステップ5にチェックマーク✓。

🎥 カメラ: プルバック（全体表示に戻る、0.3秒）

■ Phase 5（ステップ6 + 全体所要時間、最後の区間）:
ステップ6「VOICEVOX確認」がアクティブ化。
  "[6/6] Checking VOICEVOX..."
  "      VOICEVOX not found." (warning色)
ステップ6にwarning色の△マーク（VOICEVOXは次の手順で対応するため）。

全ステップ表示完了後、下部に
「所要時間: 10〜20分（ダウンロード待ちがほとんど）」
テキスト（font-size 24px、textSecondary色）がフェードインで登場。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
コードブロック・ターミナルの背景は半透明にして背景画像を透かす。
```

### Prompt 2-6: VoicevoxScene（06-voicevox）

```
src/scenes/VoicevoxScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "06-voicevox" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「VOICEVOXだけは自動インストールできないので、手動でお願いします。
voicevox.hiroshiba.jpにアクセス。
ダウンロードをクリック。
Windows版をダウンロードしてインストール。
インストールが終わったら起動。
起動すると画面が出ますが、閉じなければOKです。タスクトレイに常駐して裏で動き続けます。
VOICEVOXはスマートスピーカーを使うときに毎回起動しておく必要があります。
run.batが自動で起動を試みますが、インストールだけは先にやっておいてください。」

ビジュアル構成:

■ Phase 1（VOICEVOX紹介、最初の1文の区間）:
SectionTitle「手順3: VOICEVOXをインストール」（font-size 48px）がspring()で登場→上部へ退避。

「VOICEVOX」ロゴテキスト（font-size 56px、bold）が画面中央にspring()で
scale(0→1)ポップアップ。
下に「日本語音声合成エンジン」テキスト（font-size 24px、textSecondary）がフェードイン。
「手動インストールが必要」テキスト（font-size 20px、warning色）がslideIn(bottom)で登場。

■ Phase 2（手順リスト、「アクセス」〜「インストール」の区間）:
ロゴがtranslateY(0→-220px) + scale(1.0→0.6)で上部に退避。

画面中央にStepListコンポーネント（横幅85%）:
4ステップが順次staggeredEntrance(staggerDelay=ナレーション同期)で登場:

  Step 1: 「voicevox.hiroshiba.jp にアクセス」
    🌐アイコン + URL(accent色、monospace、font-size 24px)
    登場時spring() + slideIn(left)

  Step 2: 「ダウンロードをクリック」
    ⬇️アイコン + テキスト(font-size 24px)
    登場時spring() + slideIn(left)

  Step 3: 「Windows版をインストール」
    💻アイコン + テキスト
    登場時spring() + slideIn(left)

  Step 4: 「起動する」
    ▶️アイコン + テキスト
    登場時spring() + slideIn(left)

各ステップ登場時に前のステップにチェックマーク✓がspring()でポップアップ。

■ Phase 3（注意事項、「毎回起動しておく必要」の区間）:
ステップリストがopacity(1.0→0.4)にフェードダウン。
画面下部40%に注意カードが登場:
  warning色のボーダー（左辺3px）+ codeBg背景、横幅80%
  「⚠ 毎回起動が必要」（font-size 28px、warning色、bold）
  「run.batが自動起動を試みますが、インストールは先に」（font-size 22px、textSecondary）
  カードがslideIn(bottom) + spring()で登場。
  「毎回起動」テキスト部分がpulse(2回)で強調。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
```

### Prompt 2-7: RunScene（07-run）

```
src/scenes/RunScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "07-run" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「ここまで来たらあとは簡単です。
run.batをダブルクリックしてください。
run.batが裏側のサービスを確認・起動して、スマートスピーカー本体を立ち上げます。
この表示が出れば準備完了です。
さくらと呼びかける。ウェイクワード。
ピッという反応があったら、質問を話す。
AIが考えて、音声で返事してくれる。
例えばさくら、今日の天気教えて。さくら、カレーの作り方は。のように話しかけてみてください。
2回目以降はrun.batをダブルクリックするだけです。setup.batは最初の1回だけでOK。」

visual-onlyセグメント（ターミナル出力）:
[1/3] Checking Ollama...
      Ollama: Running
[2/3] Checking VOICEVOX...
      VOICEVOX: Running

  --- Service Status ---
  Ollama:   OK
  VOICEVOX: OK
  --------------------

[3/3] Launching Smart Speaker...

ビジュアル構成:

■ Phase 1（run.bat実行、最初の3文の区間）:
SectionTitle「手順4: run.batで起動」（font-size 48px）がspring()で登場→上部へ退避。

TerminalOutputコンポーネント（横幅90%、font-size 24px）:
  ターミナル枠がscale(0.8→1.0)でポップアップ。
  タイトルバー「Smart Speaker Launcher」。

  出力がtypewriterパターンで1行ずつ表示:
  "[1/3] Checking Ollama..." → 0.3秒待ち →
  "      Ollama: Running" (success色) →
  "[2/3] Checking VOICEVOX..." → 0.3秒待ち →
  "      VOICEVOX: Running" (success色)

  区切り線が表示された後:
  "  Ollama:   OK" (success色) + ✓マーク
  "  VOICEVOX: OK" (success色) + ✓マーク
  各OKにspring()でチェックマークポップアップ。

⚡ Status Change: 最後の "[3/3] Launching Smart Speaker..." 表示時に
  ターミナル全体にsuccess色のサブトルなglow（box-shadow）がフェードイン。

■ Phase 2（使い方、「さくらと呼びかける」以降の区間）:
ターミナルがscale(1.0→0.5) + translateY(0→-250px)で上部に縮小。

画面中央に使い方の3ステップが大きく表示:

  Step 1: 人アイコン + 吹き出し「さくら」（font-size 36px、accent色）
    slideIn(left) + spring()で登場
    吹き出しにpulse(2回)でウェイクワードを強調

  → 矢印がdrawEdgeで描画 →

  Step 2: マイクアイコン + 吹き出し「質問を話す」（font-size 28px）
    slideIn(bottom) + spring()で0.5秒遅延登場

  → 矢印がdrawEdgeで描画 →

  Step 3: スピーカーアイコン + 吹き出し「AIが音声で返事」（font-size 28px、success色）
    slideIn(right) + spring()で1.0秒遅延登場

🎥 カメラ: Step 1登場時にStep 1方向にtranslateX微パン →
  Step 3登場時にプルバックで全体表示。

■ Phase 3（2回目以降の説明、最後の1文の区間）:
使い方ステップがopacity(1.0→0.3)にフェードダウン。

画面下部に大きなテキスト:
  「2回目以降は run.bat だけ！」（font-size 40px、accent色、bold）
  spring()でscale(0.5→1.0)ポップアップ + pulse(2回)。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
コードブロック・ターミナルの背景は半透明にして背景画像を透かす。
```

### Prompt 2-8: WakewordScene（08-wakeword）

```
src/scenes/WakewordScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "08-wakeword" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「デフォルトではさくらがウェイクワードになっていますが、好きな言葉に変更できます。
ブラウザで以下のサイトにアクセスします。
自分の好きなウェイクワードをマイクに向かって3回録音します。
例えば、ねえアリス、オッケーコンピュータなど。
静かな環境でハッキリ発音するのがコツです。
録音が終わったらrpwファイルをダウンロード。
ダウンロードしたrpwファイルをスマートスピーカーのフォルダに入れる。
config/settings.tomlをメモ帳で開く。
wakewordセクションのwakeword_pathを変更する。
run.batで再起動すれば、新しいウェイクワードで動きます。
うまく反応しない場合はthresholdを0.25くらいに下げてみてください。
逆に誤反応が多い場合は0.45くらいに上げると改善します。」

visual-onlyセグメント（toml設定）:
[wakeword]
wakeword_path = "my_wakeword.rpw"   # ← ダウンロードしたファイル名に変更
threshold = 0.35
avg_threshold = 0.15
min_scores = 1

ビジュアル構成:

■ Phase 1（イントロ、最初の1文の区間）:
SectionTitle「ウェイクワードを自分好みに変える」（font-size 44px）がspring()で登場→上部へ退避。

画面中央に吹き出しが3つ横並びで表示（各幅25%、gap 3%）:
  「さくら」→「ねえアリス」→「オッケーコンピュータ」
  staggeredEntrance(staggerDelay=8)で順にspring()ポップアップ。
  各吹き出しはcodeBg背景、rounded、font-size 28px。
  「さくら」にaccent色ボーダー（デフォルト表示）。

■ Phase 2（録音手順、「ブラウザで〜」から「rpwファイルをダウンロード」の区間）:
吹き出しがopacity(1→0)でフェードアウト。

画面にブラウザ風UIフレーム（横幅90%、高さ60%）がslideIn(bottom) + spring()で登場:
  アドレスバーに URL（font-size 18px、monospace）がtypewriterで表示。
  ブラウザ内に録音UIモック:
    大きなマイクアイコン（80px、中央）がspring()で登場。
    マイク下に「1回目 / 3回」テキスト。
    ⚡ countUpパターンで 1→2→3 とカウントが進む（各1秒間隔）。
    各カウント時にマイクアイコンにpulse(1回) + 赤いrecordingドットが点滅。
  3回完了後「ダウンロード」ボタンがspring()で強調表示（accent色背景）。
  ボタンからrpwファイルアイコンがslideIn(bottom)で飛び出す演出。

🎥 カメラ: 録音UI表示中にマイクアイコンにfocusZoom（scale 1.2、0.3秒）→
  ダウンロード時にプルバック。

■ Phase 3（設定変更、「rpwファイルをフォルダに入れる」から「wakeword_pathを変更する」の区間）:
ブラウザUIがscale(1.0→0.5) + opacity(1→0.3)でフェードダウンしつつ上部へ退避。

CodeBlockコンポーネントでsettings.toml設定を表示（横幅90%、font-size 26px）:
  visibleLinesパターンで1行ずつ表示:
    [wakeword]
    wakeword_path = "my_wakeword.rpw"
    threshold = 0.35
    avg_threshold = 0.15
    min_scores = 1

  「wakeword_path」の行にaccent色のhighlightSlideIn。
  「← ダウンロードしたファイル名に変更」注釈がtextSecondary色でフェードイン。

🎥 カメラ: wakeword_path行にfocusZoom（scale 1.3、0.5秒）。

■ Phase 4（threshold調整、最後の3文の区間）:
🎥 カメラ: プルバック（scale 1.0、0.3秒）。

⚡ highlightSlideIn遷移: wakeword_path → threshold行にハイライトが移動（0.3秒）。

threshold行の横にスライダー風UIが登場（slideIn(right)）:
  「反応しない ← 0.25 ── 0.35 ── 0.45 → 誤反応する」
  スライダーのつまみがinterpolate()で0.35位置にspring()で登場。
  「反応しない」側にerror色、「誤反応する」側にwarning色のグラデーション。
  現在値0.35がfont-size 24px、accent色で表示。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
コードブロック・ターミナルの背景は半透明にして背景画像を透かす。
3つ以上のPhaseがあるので🎥カメラワーク指示を含めた。
```

### Prompt 2-9: TroubleshootScene（09-troubleshoot）

```
src/scenes/TroubleshootScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "09-troubleshoot" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「よくあるトラブルをいくつか紹介しますが、ここに載っていない問題が出たら、
遠慮なくこの動画のコメント欄で聞いてください。
エラーメッセージを貼ってもらえると対応しやすいです。
Ollama not foundと出る。setup.batを先に実行してください。
VOICEVOX Failed to startと出る。VOICEVOXをインストールして起動してからrun.batを再実行。
Whisperモデルのダウンロードが途中で止まる。再度setup.batを実行すれば途中から再開します。
ウェイクワードに反応しない。PCのマイクがオンになっているか確認してください。
反応が遅い。CPU版を使っている場合、音声認識に時間がかかります。
NVIDIA GPU搭載PCならCUDA-bundled版を試してください。
その他のトラブル。この動画のコメント欄に書いてください。
エラーメッセージをそのまま貼ってもらえると、すぐに原因がわかることが多いです。」

ビジュアル構成:

■ Phase 1（イントロ + コメント誘導、最初の3文の区間）:
SectionTitle「うまく動かないときは」（font-size 48px）がspring()で登場→上部へ退避。

画面中央に大きなコメントアイコン（💬、80px）がspring()でポップアップ。
下に「コメント欄で質問してください」テキスト（font-size 32px、accent色）が
フェードイン + pulse(2回)で強調。
さらに下に「エラーメッセージを貼ると対応しやすい」（font-size 22px、textSecondary）がフェードイン。

■ Phase 2（FAQ表示、各トラブル項目の区間）:
コメントアイコンがopacity(1→0)でフェードアウト。

FAQカードが縦に配置（横幅85%）。
各カードがstaggeredEntranceでナレーション進行に合わせて1枚ずつ登場:

  カード1「Ollama not found」:
    error色の左ボーダー3px + codeBg背景
    エラー文（font-size 22px、error色、monospace）
    → 矢印 → 解決策（font-size 20px、success色）
    slideIn(left) + spring()で登場

  カード2「VOICEVOX: Failed to start」:
    同パターン、0.5秒遅延で登場

  カード3「Whisperダウンロード停止」:
    warning色の左ボーダー
    同パターン

  カード4「ウェイクワード反応なし」:
    同パターン

  カード5「反応が遅い」:
    同パターン

🎥 カメラ: カードが増えるごとにtranslateYで上スクロール（パン）。
  全5枚が画面高さの130%で縦配置。
  各カード説明時に該当カードが画面中央に来るようスクロール。

■ Phase 3（コメント誘導の再強調、「その他のトラブル」の区間）:
全カードがopacity(1.0→0.4)にフェードダウン。

画面中央に強調カード（accent色ボーダー、横幅80%）がspring()で登場:
  「その他のトラブル → コメント欄へ」（font-size 32px、accent色、bold）
  「エラーメッセージをそのまま貼ってください」（font-size 22px、textSecondary）
  pulse(3回)で強調。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
コードブロック・ターミナルの背景は半透明にして背景画像を透かす。
```

### Prompt 2-10: SummaryScene（10-summary）

```
src/scenes/SummaryScene.tsx を作成して。

背景: public/images/background.pngを全画面に表示（staticFile使用）。
背景の上にコンテンツラッパーdivを配置し、カメラワークはラッパーに適用する。
このシーンはscript-aligned.jsonのsceneId "10-summary" に対応する。
フレーム数はscene-frames.jsonから読み取る。

ナレーションの流れ:
「もう一度手順をおさらいします。
7zファイルを展開。
setup.batを実行。初回のみ、10〜20分。
VOICEVOXをインストール。手動、初回のみ。
run.batを実行。毎回これだけ。
さくらと話しかける。
何かつまずいたら、コメント欄にエラーメッセージと一緒に書き込んでください。
できる限り対応します。
それでは、楽しんで使ってもらえたら嬉しいです！」

ビジュアル構成:

■ Phase 1（おさらいリスト、「おさらい」〜「さくらと話しかける」の区間）:
SectionTitle「まとめ」（font-size 48px）がspring()で登場→
0.3秒後にtranslateY(0→-280px)で上部へ退避。

画面中央にチェックリスト（横幅85%）:
5ステップが順番にstaggeredEntrance(staggerDelay=ナレーション同期)で登場:

  ☐ 1. 7zファイルを展開
  ☐ 2. setup.bat を実行（初回のみ）
  ☐ 3. VOICEVOXをインストール（初回のみ）
  ☐ 4. run.bat を実行
  ☐ 5.「さくら」と話しかける

各ステップはfont-size 30px。
ナレーションが各ステップに言及するタイミングで:
  ☐ → ☑ にspring()でチェックマークがポップアップ。
  チェック時にsuccess色がフェードイン。
  チェック済みのステップはopacity 0.7にフェードダウン。

ステップ4「run.bat を実行」に「毎回これだけ！」バッジ（accent色背景、font-size 16px）が
spring()でポップアップ。

■ Phase 2（コメント誘導 + エンディング、最後の3文の区間）:
チェックリストがscale(1.0→0.7) + translateY(0→-150px)で上部へ縮小移動。

画面下部60%にエンディングカード:
  accent色のサブトルなボーダー + codeBg背景、横幅80%、padding 30px
  slideIn(bottom) + spring()で登場。

  「つまずいたらコメント欄へ！」（font-size 36px、accent色、bold）
  pulse(2回)で強調。

  下に「エラーメッセージを一緒に貼ってください」（font-size 22px、textSecondary）
  フェードイン。

  最後に「楽しんで使ってもらえたら嬉しいです！」（font-size 28px、textPrimary）が
  spring()でフェードイン。全体にsuccess色のサブトルなglowが広がる。

アニメーションはナレーションの単語タイムスタンプに同期させる。
画面上に字幕は表示しない。ナレーションは音声のみ。
全ての要素は画面の85%以上の幅を使うこと。
```

---

## Phase 3: シーン結合

### Prompt 3-1: Video.tsx 結合

```
src/Video.tsx を作成して。

以下の構造で全シーンを結合する:

1. data/script-aligned.json をpropsとして受け取る
2. 単一の <Audio src={staticFile('audio/full.wav')} /> を配置
3. 各シーンをscene-frames.jsonのフレーム情報で <Sequence> に配置
4. CUSTOM_SCENES マッピングで各シーンIDと対応するコンポーネントを登録:

import { OpeningScene } from "./scenes/OpeningScene";
import { OverviewScene } from "./scenes/OverviewScene";
import { VersionSelectScene } from "./scenes/VersionSelectScene";
import { ExtractScene } from "./scenes/ExtractScene";
import { SetupScene } from "./scenes/SetupScene";
import { VoicevoxScene } from "./scenes/VoicevoxScene";
import { RunScene } from "./scenes/RunScene";
import { WakewordScene } from "./scenes/WakewordScene";
import { TroubleshootScene } from "./scenes/TroubleshootScene";
import { SummaryScene } from "./scenes/SummaryScene";

const CUSTOM_SCENES: Record<string, React.FC> = {
  "01-opening": OpeningScene,
  "02-overview": OverviewScene,
  "03-version-select": VersionSelectScene,
  "04-extract": ExtractScene,
  "05-setup": SetupScene,
  "06-voicevox": VoicevoxScene,
  "07-run": RunScene,
  "08-wakeword": WakewordScene,
  "09-troubleshoot": TroubleshootScene,
  "10-summary": SummaryScene,
};

5. フォールバック用の汎用Sceneコンポーネント（CUSTOM_SCENESにないシーン用）

画面下部にProgressBarを全シーン共通で配置（全体の進行度を表示）。
```

### Prompt 3-2: トランジション

```
src/components/TransitionEffect.tsx を作成して。

シーン間のトランジションエフェクト:
- フェードイン/アウト（opacity 0→1→0、各10フレーム）
- シーン切り替え時に適用
- Audio要素には影響しない（視覚のみ）

Video.tsx の各 <Sequence> にトランジションを組み込む。
前のシーンの最後10フレームでフェードアウト、
次のシーンの最初10フレームでフェードイン。
```

---

## Phase 4: 仕上げ

### Prompt 4-1: BGM・効果音

```
Audio関連の仕上げ:

1. BGM（任意）:
   public/audio/bgm.mp3 を配置した場合、
   <Audio src={staticFile('audio/bgm.mp3')} volume={0.08} />
   で全体に薄くBGMを流す。

2. 効果音（任意）:
   ウェイクワード検出時の「ピッ」音など、
   必要に応じてpublic/audio/se/ にSEファイルを配置。
```

### Prompt 4-2: 微調整

```
以下の最終調整を行って:

1. 全シーンのアニメーションタイミングをRemotionStudioで確認
2. ナレーションとビジュアルのズレがある箇所を修正
3. フォントサイズ・配置の一貫性を確認
4. 全てのspring()アニメーションのdamping/stiffnessが統一されているか確認
5. 背景画像が全シーンで正しく表示されているか確認
6. ProgressBarが全シーン通じて正しく進行しているか確認
```

---

## 補足: よくある修正プロンプト

1. **ターミナル出力のタイミングがズレている**
   「SetupSceneのターミナル出力が早すぎる。[3/6]のOllamaインストールの行が表示されるタイミングをナレーションの「Ollamaのインストール」に合わせて。script-aligned.jsonのword timestampを参照して。」

2. **カードが小さすぎる**
   「VersionSelectSceneの3枚のカードが横幅75%しか使っていない。画面幅の90%を使うように各カード幅を30%→28%にし、全体を横幅90%の中央配置に変更して。font-sizeも20%拡大して。」

3. **アニメーションが静的になっている**
   「TroubleshootSceneのFAQカードがただ表示されているだけになっている。各カードにslideIn(left) + spring()の登場アニメーションを追加し、カード間の切り替え時にrackFocus（前カードopacity低下、新カードopacity上昇）を入れて。」

4. **カメラワークがない**
   「WakewordSceneの4つのPhaseでカメラが一切動いていない。Phase 2の録音UIにfocusZoom(scale 1.2)を追加し、Phase 3のtoml表示にfocusZoom(scale 1.3)でwakeword_path行にフォーカス、Phase 4でpullBackして全体表示に戻して。」

5. **コメント誘導が弱い**
   「SummarySceneのエンディングカードにもっと強い視覚的強調が欲しい。コメントアイコンをpulse(3回)に増やし、accent色のglowをbox-shadow: 0 0 30px rgba(88,166,255,0.4)に強化して。」

---

## プロンプト設計の原則

1. **1プロンプト = 1ファイル**
   1回のプロンプトで1つのシーンファイル（例: `src/scenes/SetupScene.tsx`）だけを作る。複数ファイルの同時作成は避ける。

2. **フレーム数はデータから**
   フレーム数をプロンプトにハードコードしない。`scene-frames.json` から取得する指示にする。例: 「このシーンのフレーム数はscene-frames.jsonから読み取る」。

3. **全Phaseにアニメーション指示を必ず含める（静的表示禁止）**
   「カードを表示」ではなく「カードがspring()でscale(0→1)ポップアップ登場」のように、全てのPhaseに動きの指示を含める。

4. **全要素にサイズを明記（width 85%以上、font-size指定）**
   「テキストを表示」ではなく「font-size 28px、横幅90%、中央配置でテキストを表示」。サイズ指定なしだとClaude Codeは小さく配置する。

5. **視覚的な説明を添える（Remotion関数名を含める）**
   「登場する」→「spring({ damping: 12 })でscale(0→1)ポップアップ」。interpolate(), spring(), Easing.bezier()を明示的に使う。

6. **アニメーションパターンを名前で参照**
   `typewriter`, `visibleLines`, `staggeredEntrance`, `focusZoom`, `countUp`, `colorMorph`, `pulse`, `slideIn`, `drawEdge`などをパターン名で指示する。

7. **既存ファイルとの関係を明示**
   「ComparisonCardsコンポーネント（src/components/ComparisonCards.tsx）を使用」のように、参照先を明記する。

---

## レンダリング指示

```bash
# プレビュー
npx remotion studio

# レンダリング（propsとしてscript-aligned.jsonを渡す）
npx remotion render --props=data/script-aligned.json Video out/setup-guide.mp4

# 特定シーンだけプレビュー（フレーム範囲指定）
npx remotion render --props=data/script-aligned.json --frames=0-900 Video out/preview-opening.mp4
```

---

## CLAUDE.md テンプレート

プロジェクトルートに以下の `CLAUDE.md` を配置してください：

```markdown
# Remotion プロジェクト — AIスマートスピーカー セットアップガイド

## タイミング
- All timing derived from script-aligned.json
- scene-frames.json has per-scene frame counts
- Never hardcode frame numbers — always reference JSON data

## 音声
- Code blocks in script.md are visual-only, not narrated
- No subtitles on screen — narration is audio only
- Single Audio element in Video.tsx, timestamp-driven Sequences

## ビジュアル
- Background: always use staticFile('images/background.png'), never generate backgrounds
- Camera transforms apply to content wrapper only, not background
- All elements must use 85%+ screen width
- Code/terminal backgrounds are semi-transparent (codeBg/terminalBg from config)

## アニメーション
- Every Phase must include spring()/interpolate() animation instructions
- Use animation recipes: CodeBlock, Terminal, BeforeAfter, Diagram, Debug
- Use 🎥 camera work (focusZoom, pullBack, pan, rackFocus) for scenes with 3+ phases
- Use ⚡ state transitions (countUp, inline code fix, panel transform, status change) for changes
- Use animation utility patterns from src/utils/animations.ts

## コンポーネント
- Scene components in src/scenes/ — one per scene
- Shared components in src/components/
- All scenes registered in CUSTOM_SCENES map in Video.tsx
```

---

## 作業フロー全体のまとめ

```
1. CLAUDE.mdを配置
2. data/input/ にシーンごとの台本(.md)と音声(.wav)を配置
3. Phase 0: 音声パイプライン実行
   → parse-scripts → concat-audio → align-timestamps → compute-frames
   → scene-frames.json の生成を確認
4. Phase 1のプロンプトを実行（3プロンプト）
5. npx remotion studio で基盤確認
6. Phase 2を1シーンずつ実行
   → 各シーン作成後に音声同期をRemotionStudioで確認
   → ズレがあればscript-aligned.jsonのタイムスタンプを確認
7. Phase 3で結合・トランジション
8. Phase 4で仕上げ
9. npx remotion render で最終出力
```
