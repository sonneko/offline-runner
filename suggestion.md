# iOS PWAエンジニア向け万能ツール 設計提案書 (詳細版)

## 1. 概要
本プロジェクトは、iPadOS上のSafari（PWA）において、オフライン環境でもエンジニアが汎用的に問題解決を行える「超軽量・高性能」なツール群を設計する。
iOS 16.4以降のOrigin Private File System (OPFS) とWebAssembly (Rust) を中核に据え、OSの制約を回避しつつ、デスクトップ級の操作体験を提供することを目指す。

## 2. 詳細システム構成案

### 2.1. 階層化アーキテクチャ
「動作の激軽さ」を担保するため、以下の3層構造とする。

1.  **UI Layer (Svelte + TS)**: 描画に徹する。ステート管理は最小限とし、重い処理の結果を受け取るだけのリアクティブなフロントエンド。
2.  **Orchestrator Layer (Web Worker + Comlink)**: JSで記述。Rust/Wasmのライフサイクル管理、UIからのRPC受け付け、OPFSの非同期ハンドル（メインスレッド用）と同期ハンドル（Rust用）の橋渡しを行う。
3.  **Engine Layer (Rust + Wasm)**: 実際のロジック（ファイル解析、シェルエミュレーション、スクリプト実行）。`SyncAccessHandle` を通じてOPFSに直接、同期的にアクセスする。

### 2.2. OPFS管理とI/O戦略
iOS Safariのメモリ・ストレージ制限を考慮した実装。
- **SyncAccessHandle Pooling**: `createSyncAccessHandle()` は高コストなため、Worker起動時に主要ファイル用のハンドルをプールし、使い回す。
- **Byte-Range Reading**: 巨大なファイル（ログ、PDF）を扱う際、ファイル全体をメモリにロードせず、Rust側のポインタ移動で必要なバイト範囲のみを `read()` する。これにより、数百MBのファイルでも低メモリ消費で即座に開ける。
- **Quota Management**: `navigator.storage.estimate()` を定期的に監視し、ストレージ容量が逼迫した際の自動クリーンアップ（一時ディレクトリの削除）を実装。

## 3. 機能を支える具体的仕様

### 3.1. 仮想ファイルシステム (VFS) と CLI 詳細
Rust側で構築する独自のVFSレイヤーにより、OSのファイルシステム制限を超えた操作を実現する。

- **VFS構成**:
    - **Root**: `/` をOPFSのルートにマッピング。
    - **Mount Point**: `/mnt/icloud` (File Picker経由の外部アクセス), `/dev/null`, `/tmp` (メモリ内KVS) などを仮想的に提供。
- **コマンド実装詳細**:
    - `ls [-l, -a]`: `FileSystemDirectoryHandle.values()` をイテレートし、Rust側でメタデータ（サイズ、最終更新）を整形。
    - `grep`: ファイルを `chunk` 単位で読み込み、マルチスレッド（WebWorkerの並列化）で正規表現検索を実行。
    - `find`: 再帰的なハンドル取得をスタックベースで実装し、メモリ消費を抑制。
    - `xargs`: 前段のコマンド出力をパースし、逐次コマンド実行。
- **ターミナル実装 (iPad最適化)**:
    - **Input Buffer**: ソフトウェアキーボードの「確定」イベントを待たずに1文字ずつRust側に送り、リアクティブな補完を実現。
    - **Special Key Emulation**: 画面端に `Ctrl`, `Esc`, `Tab`, `Shift` などの仮想キーを配置し、キーボードなしのiPad操作を補完する。

### 3.2. Mini-ShellScript (MSS) 言語仕様・実装詳細
エンジニアが現場で「10行程度の自動化」を即座に記述・実行するためのドメイン特化言語。

- **詳細文法 (EBNF)**:
    ```ebnf
    program      = { statement } ;
    statement    = assignment | if_stmt | for_stmt | command_call | func_def ;
    assignment   = "$" , identifier , "=" , ( expression | backtick_cmd ) ;
    if_stmt      = "if" , condition , "{" , program , "}" , [ "else" , "{" , program , "}" ] ;
    for_stmt     = "for" , "$" , identifier , "in" , list_expr , "{" , program , "}" ;
    command_call = "@" , command_name , { arg } ;
    backtick_cmd = "`" , command_call , "`" ; (* コマンド出力を変数に代入 *)
    arg          = string | "$" , identifier | backtick_cmd ;
    ```
- **ランタイム仕様**:
    - **変数スコープ**: グローバルスコープのみ。関数内変数は動的スコープ（実装簡略化のため）。
    - **組み込み関数**: `print()`, `len()`, `sleep()`, `http_get()`, `write_file()`, `read_file()`。
    - **エラーハンドリング**: スクリプト全体を `try-catch` 的な構造でRust側がラップし、エラー発生時はスタックトレースとファイル位置（行・列）を表示。
- **実装アプローチ**:
    - **Lexer/Parser**: Rustの `logos` (Lexer) と `nom` (Parser) クレートを使用し、バイナリサイズを抑えつつ高速なパースを実現。
    - **Interpreter**: `Box<Expr>` を再帰的に評価する AST Walker。各 `statement` 実行前に Worker の `postMessage` で進捗をUIに通知し、長時間実行時の「応答なし」を防ぐ。
- **実行方法とトリガー**:
    - **CLI起動**: `mss run script.mss` コマンドによる直接実行。
    - **エディタ連携**: エディタ上の「Run」ボタンからカレントファイルを実行。
    - **フック実行**: 特定のファイル保存時や、PWA起動時に自動実行される「スタートアップスクリプト」の設定。

- **スクリプト例 (MSS)**:
    ```bash
    # ログから特定の行を抽出してSQLiteに保存する例
    $target = "error.log"
    $output = ` @grep "ERROR" $target `

    @ls -l $target

    if $output {
        @print "Errors found. Saving to DB..."
        @sqlite "INSERT INTO logs (content) VALUES ('" + $output + "')"
    }
    ```

### 3.3. SQLite Wasm & Mermaid.js / 機械翻訳 実装詳細

#### 3.3.1. SQLite Wasm (OPFS VFS)
- **VFS構成**: SQLite公式の `opfs-sah-pool` VFSを採用。
    - **SAH Pool**: 複数の `FileSystemSyncAccessHandle` を事前に確保（プール）し、SQLiteのページ書き込み要求に対して動的に割り当てる。これにより、Safariでのハンドル生成オーバーヘッドを回避。
- **Worker通信構成**:
    - メインスレッドからはSQL文字列を送り、Worker側で `Uint8Array` のレコードセットを受け取るストリーム形式。
    - 大容量データの取得時は、`SharedArrayBuffer` を介したゼロコピー転送を行い、UIのプチフリーズを防止。

#### 3.3.2. Mermaid.js 描画パイプライン
- **Headless Rendering**: `Mermaid.js` をメインスレッドで動かすとDOM操作が重いため、隠し iframe 内でレンダリングするか、可能であれば `mermaid-cli` 相当のロジックを Worker 内の `OffscreenCanvas` (2D操作のみ) でエミュレートする。
- **Cache & Preview**:
    - CodeMirrorの入力が止まってから 500ms 後にレンダリング。
    - 生成されたSVGはOPFSの `.cache/mermaid/` にハッシュ値名で保存し、次回以降の高速表示に利用。

#### 3.3.3. 機械翻訳 (Machine Translation)
- **実装方法**:
    - **Online**: DeepL API または Google Cloud Translation API を利用したシンプルなプロキシ。
    - **Offline**: `WebLLM` または `Transformers.js` を利用し、Worker内で小規模な言語モデル（Llama-3-8B等）を動かす。iPadのGPU (Metal/WebGPU) を活用し、オフラインでも高い翻訳精度を実現。
    - 翻訳結果はエディタのサイドパネルに表示し、ワンクリックでコード内への挿入を可能にする。

### 3.4. PDF/Text プレビュー実装
- **PDF**:
    - **基本表示**: `URL.createObjectURL` で生成したBlob URLを `<iframe>` に流し込む。
    - **テキスト検索**: `pdf.js` のコアロジックのみをWorkerで動かし、テキストレイヤーのインデックスを作成。CLIの `grep` コマンドからPDF内の文字列検索を可能にする。
    - **付箋・注釈 (Annotation)**:
        - ブラウザ標準のインライン表示では注釈編集が難しいため、`PDF-Lib` をWorker側で動かし、座標指定でテキストや図形を合成する。
        - 編集中の「付箋」データは一時的にJSONとしてOPFSに保存し、保存（Save）時にPDF本体へ統合（Flattening）する。
- **Editor**:
    - CodeMirror 6 を「Headless」に近い状態で利用。シンタックスハイライト（Tree-sitter Wasm等）はWorkerで計算し、差分のみをCM6の `Decoration` として適用することで、超巨大ファイルの編集負荷を分散。

## 4. フロントエンド・バックエンド通信とiPad最適化

### 4.1. Worker通信プロトコル (Message Schema)
UIスレッドとWorker間のやり取りを構造化する。

- **Request/Response 型定義**:
    ```typescript
    type Request =
      | { type: 'EXEC_CMD', cmd: string, args: string[] }
      | { type: 'READ_FILE', path: string, offset: number, length: number }
      | { type: 'SQL_QUERY', sql: string }
      | { type: 'RUN_SCRIPT', code: string };

    type Response =
      | { type: 'STDOUT', data: string }
      | { type: 'FILE_DATA', buffer: ArrayBuffer }
      | { type: 'ERROR', message: string, code: number };
    ```
- **転送の最適化**: 巨大なファイルデータは `Transferable Objects` を利用して所有権を移動させ、コピーコストをゼロにする。

### 4.2. iPad固有のUX・パフォーマンス設計
- **レイアウトとUIコンポーネント**:
    - **3ペイン構成**: 左にファイルツリー、中央にエディタ/プレビュー、右にターミナル/翻訳パネルを配置。各ペインはスプリッターで自由にサイズ調整可能。
    - **コマンドパレット**: `Cmd+P` で起動。全機能（ファイル検索、コマンド実行、MSS呼び出し）へキーボードのみでアクセス可能にする。
- **Keyboard Shortcuts**: `Cmd+S` (Save), `Cmd+P` (Command Palette), `Ctrl+C` (Interrupt) 等のハンドリングをJS側でグローバルに補足。
- **Focus Management**: ターミナルからエディタ、あるいは検索バーへの移動をタブキーで循環できるよう `tabindex` を動的に管理。
- **PWA Lifecycle**: iPadOSがバックグラウンドでPWAをサスペンドさせた際の状態保存（セッション復元）を `IndexedDB` に定期的にオートセーブ。
- **Apple Pencil サポート**: Canvas APIを用いた手書きメモ機能。Mermaid.jsの生成図に「手書き」で注釈を加えるレイヤーを実装。

## 5. オフライン・デプロイ戦略
- **Service Worker (Workbox)**:
    - `runtimeCaching` を設定し、`*.wasm` や `xterm.js` などの静的アセットを `CacheFirst` で保持。
    - **Offline Sync**: オフライン中に編集したメタデータを記録し、オンライン復帰時に任意のリモート（GitHub等）と同期するフックを用意。
- **PWA Manifest**: `display: standalone` かつ `orientation: landscape` を推奨し、iPadを横向きで「ラップトップ」のように使う体験を固定。
