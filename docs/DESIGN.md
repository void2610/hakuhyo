# Hakuhyo - Discord TUIクライアント 詳細設計書

## プロジェクト概要

**プロジェクト名**: Hakuhyo (薄氷)
**目的**: Rustで軽量なDiscord TUIクライアントを作成
**方針**: Discord用ライブラリを使用せず、REST APIとWebSocket Gatewayを直接実装

### 実装する機能

1. **チャンネル/DM一覧表示**
   - ギルドチャンネル一覧
   - DMチャンネル一覧
   - リストから選択

2. **メッセージ表示**
   - 選択したチャンネルのメッセージを表示
   - アイコンや画像プレビューは無し
   - テキストのみのシンプル表示

3. **メッセージ送信**
   - テキストメッセージの送信
   - 添付ファイルや画像は無し

4. **リアルタイム受信**
   - WebSocket Gatewayで新着メッセージをリアルタイム表示

### 実装しない機能

- アイコン・画像プレビュー
- 添付ファイル送信
- メンション・リアクション
- 絵文字表示
- マークダウンレンダリング
- 複数ギルド切り替え（1つに限定）
- メッセージ編集・削除
- スレッド機能

---

## 技術スタック

### 依存ライブラリ

```toml
[dependencies]
# TUI
ratatui = "0.28"               # TUIフレームワーク
crossterm = "0.28"             # クロスプラットフォームターミナル操作

# HTTP & WebSocket（ライブラリなし・直接実装）
reqwest = "0.12"               # HTTP クライアント
tokio-tungstenite = "0.20"     # WebSocket クライアント

# 非同期ランタイム
tokio = "1"                    # 非同期ランタイム
futures = "0.3"                # Futuresユーティリティ

# JSON処理
serde = "1.0"                  # シリアライゼーション
serde_json = "1.0"             # JSON サポート

# エラーハンドリング
anyhow = "1.0"                 # エラー処理

# ユーティリティ
chrono = "0.4"                 # 日時処理
```

### 選定理由

- **Ratatui**: 軽量で柔軟、非同期処理との統合が容易
- **crossterm**: クロスプラットフォーム対応のターミナルバックエンド
- **reqwest**: 非同期HTTP、JSON対応、広く使われている
- **tokio-tungstenite**: WebSocket、tokio統合
- **tokio**: 最も成熟した非同期ランタイム

---

## アーキテクチャ設計

### The Elm Architecture (TEA) パターン

```
┌──────────────────────────────────────┐
│           User Input                  │
│      (Keyboard, Gateway Events)       │
└─────────────┬────────────────────────┘
              │
              ▼
┌──────────────────────────────────────┐
│           Update Logic                │
│    (State + Event → New State)        │
└─────────────┬────────────────────────┘
              │
              ▼
┌──────────────────────────────────────┐
│          View Rendering               │
│         (State → UI)                  │
└──────────────────────────────────────┘
```

### 主要コンポーネント

1. **Model (State)**
   - アプリケーション全体の状態を保持
   - Discord状態（チャンネル、メッセージ）
   - UI状態（選択、入力モード）

2. **Update**
   - イベントを受け取り状態を更新
   - 副作用（API呼び出し）をコマンドとして返す

3. **View**
   - 現在の状態を元にTUIを描画
   - Ratatuiでレイアウトとウィジェット構築

---

## モジュール構成

```
src/
├── main.rs              # エントリーポイント、メインループ
├── app.rs               # アプリケーション状態管理（Model + Update）
├── ui.rs                # TUI描画ロジック（View）
├── events.rs            # イベント定義
└── discord/
    ├── mod.rs           # モジュール宣言
    ├── models.rs        # データモデル（User, Message, Channel等）
    ├── rest.rs          # REST API実装
    └── gateway.rs       # WebSocket Gateway実装
```

### 各モジュールの責務

#### `main.rs`
- アプリケーションのエントリーポイント
- 非同期メインループ
- イベント多重化（tokio::select!）
- ターミナル初期化/クリーンアップ

#### `app.rs`
- アプリケーション状態の定義
- イベントハンドリングロジック
- 状態更新ロジック

#### `ui.rs`
- TUIレイアウト定義
- ウィジェット描画
- スタイリング

#### `events.rs`
- イベント型定義（UIイベント、Discord イベント）

#### `discord/models.rs`
- Discord APIのデータ構造
- User, Message, Channel, Guild等

#### `discord/rest.rs`
- Discord REST API クライアント
- HTTP リクエスト実装
- レート制限処理

#### `discord/gateway.rs`
- Discord Gateway（WebSocket）クライアント
- 接続、認証、ハートビート
- イベント受信・デコード

---

## データモデル設計

### 主要な構造体

```rust
// ユーザー情報
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    pub avatar: Option<String>,
}

// メッセージ情報
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub author: User,
    pub content: String,
    pub timestamp: String,
    pub edited_timestamp: Option<String>,
}

// チャンネル情報
pub struct Channel {
    pub id: String,
    pub channel_type: u8,  // 0: テキスト, 1: DM, 2: ボイス
    pub guild_id: Option<String>,
    pub name: Option<String>,
    pub recipients: Option<Vec<User>>,  // DM用
}

// ギルド情報
pub struct Guild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub owner_id: String,
}
```

### アプリケーション状態

```rust
// アプリケーション全体の状態
pub struct AppState {
    pub discord: DiscordState,
    pub ui: UiState,
}

// Discord関連の状態
pub struct DiscordState {
    pub guilds: HashMap<String, Guild>,
    pub channels: HashMap<String, Channel>,
    pub messages: HashMap<String, Vec<Message>>,  // channel_id → messages
    pub current_user: Option<User>,
    pub connected: bool,
}

// UI関連の状態
pub struct UiState {
    pub selected_channel: Option<String>,
    pub channel_list_state: ListState,
    pub message_list_state: ListState,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub cursor_position: usize,
}

pub enum InputMode {
    Normal,   // ナビゲーションモード
    Editing,  // 入力モード
}
```

### イベント定義

```rust
// アプリケーションイベント
pub enum AppEvent {
    // UI イベント
    KeyPress(KeyCode),
    Input(char),

    // Discord イベント（Gateway）
    GatewayReady(ReadyData),
    MessageCreate(Message),
    MessageUpdate { id: String, channel_id: String, content: String },
    MessageDelete { id: String, channel_id: String },

    // コマンド完了イベント
    ChannelsLoaded(Vec<Channel>),
    MessagesLoaded { channel_id: String, messages: Vec<Message> },
    MessageSent(Message),

    // システムイベント
    Tick,
    Quit,
}
```

---

## Discord API実装詳細

### REST API 実装 (`discord/rest.rs`)

#### エンドポイント一覧

| 機能            | メソッド | エンドポイント                                    |
|---------------|------|--------------------------------------------|
| ギルド一覧取得       | GET  | `/users/@me/guilds`                        |
| チャンネル一覧取得     | GET  | `/guilds/{guild_id}/channels`              |
| DM一覧取得        | GET  | `/users/@me/channels`                      |
| メッセージ取得       | GET  | `/channels/{channel_id}/messages?limit=50` |
| メッセージ送信       | POST | `/channels/{channel_id}/messages`          |
| 現在のユーザー取得     | GET  | `/users/@me`                               |
| Gateway URL取得 | GET  | `/gateway/bot`                             |

#### 実装構造

```rust
pub struct DiscordRestClient {
    client: reqwest::Client,
    token: String,
    base_url: String,
}

impl DiscordRestClient {
    pub fn new(token: String) -> Self;

    // ギルド操作
    pub async fn get_guilds(&self) -> Result<Vec<Guild>>;

    // チャンネル操作
    pub async fn get_guild_channels(&self, guild_id: &str) -> Result<Vec<Channel>>;
    pub async fn get_dm_channels(&self) -> Result<Vec<Channel>>;

    // メッセージ操作
    pub async fn get_messages(&self, channel_id: &str, limit: u8) -> Result<Vec<Message>>;
    pub async fn send_message(&self, channel_id: &str, content: &str) -> Result<Message>;

    // ユーザー操作
    pub async fn get_current_user(&self) -> Result<User>;

    // Gateway
    pub async fn get_gateway_url(&self) -> Result<String>;
}
```

#### 認証

全てのリクエストにBotトークンを含める：

```rust
.header("Authorization", format!("Bot {}", token))
.header("User-Agent", "Hakuhyo/1.0")
```

#### レート制限対策

シンプルな実装として、リクエスト間に最小間隔を設ける：

```rust
// グローバルレート制限: 50 req/sec
// 安全マージンを考慮して 20ms (50 req/sec)
tokio::time::sleep(Duration::from_millis(20)).await;
```

将来的な改善：
- レスポンスヘッダー監視
- バケット単位の制限管理
- Exponential backoff

---

### WebSocket Gateway 実装 (`discord/gateway.rs`)

#### 接続フロー

```
1. Gateway URL取得
   ↓
2. WebSocket接続 (wss://gateway.discord.gg/?v=10&encoding=json)
   ↓
3. Hello受信 (Opcode 10)
   ↓
4. Identify送信 (Opcode 2)
   ↓
5. Ready受信 (Opcode 0, Event: READY)
   ↓
6. ハートビート開始 (定期的にOpcode 1を送信)
   ↓
7. イベント受信ループ
```

#### Opcode一覧

| Opcode | 名前            | 送受信 | 説明              |
|--------|---------------|-----|-----------------|
| 0      | Dispatch      | 受信  | イベント配信          |
| 1      | Heartbeat     | 送信  | ハートビート          |
| 2      | Identify      | 送信  | 接続時の認証          |
| 10     | Hello         | 受信  | 接続成功、ハートビート間隔通知 |
| 11     | Heartbeat ACK | 受信  | ハートビート確認        |

#### インテント設定

必要なインテント：

```rust
const GUILDS: u32 = 1 << 0;                    // ギルド情報
const GUILD_MESSAGES: u32 = 1 << 9;            // ギルドメッセージ
const DIRECT_MESSAGES: u32 = 1 << 12;          // DM
const MESSAGE_CONTENT: u32 = 1 << 15;          // メッセージ内容（特権）

let intents = GUILDS | GUILD_MESSAGES | DIRECT_MESSAGES | MESSAGE_CONTENT;
```

**注意**: `MESSAGE_CONTENT` は特権インテントのため、Discord Developer Portalで有効化が必要。

#### ハートビート処理

```rust
// 別タスクでハートビートを定期送信
async fn heartbeat_loop(
    ws_write: SplitSink<WebSocketStream, Message>,
    interval_ms: u64,
    last_sequence: Arc<RwLock<Option<u64>>>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(interval_ms));

    loop {
        interval.tick().await;

        let seq = *last_sequence.read().await;
        let payload = json!({
            "op": 1,
            "d": seq
        });

        ws_write.send(Message::Text(payload.to_string())).await;
    }
}
```

#### 実装構造

```rust
pub struct GatewayClient {
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    token: String,
    intents: u32,
    last_sequence: Arc<RwLock<Option<u64>>>,
    session_id: Option<String>,
}

impl GatewayClient {
    pub async fn connect(token: String, gateway_url: String) -> Result<Self>;
    pub async fn identify(&mut self) -> Result<()>;
    pub async fn start_heartbeat(&self, interval_ms: u64);
    pub async fn receive_event(&mut self) -> Result<Option<AppEvent>>;
}
```

#### 主要イベント

- **READY**: 接続完了、ユーザー情報・ギルド一覧取得
- **MESSAGE_CREATE**: 新規メッセージ
- **MESSAGE_UPDATE**: メッセージ編集
- **MESSAGE_DELETE**: メッセージ削除
- **CHANNEL_CREATE/UPDATE/DELETE**: チャンネル変更

---

## TUI設計

### レイアウト

```
┌─────────────────────────────────────────────────────────────┐
│                      Hakuhyo - Discord TUI                  │
├──────────────────┬──────────────────────────────────────────┤
│                  │                                          │
│  Channels/DMs    │         Messages                         │
│                  │                                          │
│  # general       │  [12:34] user1: Hello!                   │
│  # random        │  [12:35] user2: Hi there                 │
│  @ user123       │  [12:36] user1: How are you?             │
│  @ user456       │                                          │
│                  │                                          │
│                  │                                          │
│                  │                                          │
│                  │                                          │
│                  ├──────────────────────────────────────────┤
│                  │  Input: _                                │
│                  │                                          │
├──────────────────┴──────────────────────────────────────────┤
│  Connected | i: Edit | Esc: Normal | Enter: Send | q: Quit │
└─────────────────────────────────────────────────────────────┘
```

### レイアウト比率

- 左サイドバー（チャンネル一覧）: 20%
- 右エリア: 80%
  - メッセージ表示エリア: 可変（最小3行）
  - 入力エリア: 3行固定

### ウィジェット

1. **チャンネルリスト** (`List`)
   - チャンネル名とプレフィックス表示
   - 選択状態のハイライト
   - 上下キーでナビゲーション

2. **メッセージリスト** (`List`)
   - タイムスタンプ + ユーザー名 + 内容
   - 自動スクロール（最新メッセージを表示）

3. **入力フィールド** (`Paragraph`)
   - 編集モード時にカーソル表示
   - 複数行対応（将来的に）

4. **ステータスバー** (`Paragraph`)
   - 接続状態
   - キーバインドヘルプ

### キーバインド

| キー             | モード     | 動作                |
|----------------|---------|-------------------|
| `↑` / `k`      | Normal  | 上のチャンネルを選択        |
| `↓` / `j`      | Normal  | 下のチャンネルを選択        |
| `Enter`        | Normal  | チャンネル選択確定・メッセージ送信 |
| `i`            | Normal  | 入力モード開始           |
| `Esc`          | Editing | Normalモードに戻る      |
| `Ctrl+C` / `q` | Normal  | 終了                |
| 文字キー           | Editing | 文字入力              |
| `Backspace`    | Editing | 文字削除              |

---

## メインループ設計

### イベント駆動アーキテクチャ

```rust
#[tokio::main]
async fn main() -> Result<()> {
    // 初期化
    let mut terminal = setup_terminal()?;
    let mut app = AppState::new();

    // チャンネル作成
    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(100);

    // Discord クライアント初期化
    let rest_client = DiscordRestClient::new(token.clone());
    let gateway_client = GatewayClient::connect(token, gateway_url).await?;

    // 1. UI イベントハンドラ（別タスク）
    spawn_ui_event_handler(event_tx.clone());

    // 2. Gateway イベントハンドラ（別タスク）
    spawn_gateway_event_handler(gateway_client, event_tx.clone());

    // 3. 描画タイマー（別タスク）
    spawn_tick_timer(event_tx.clone());

    // メインループ
    loop {
        // UI描画
        terminal.draw(|f| ui::render(f, &app))?;

        // イベント処理
        if let Some(event) = event_rx.recv().await {
            if matches!(event, AppEvent::Quit) {
                break;
            }

            // 状態更新 + コマンド生成
            if let Some(cmd) = app.update(event) {
                execute_command(cmd, &rest_client, event_tx.clone()).await?;
            }
        }
    }

    // クリーンアップ
    restore_terminal(&mut terminal)?;
    Ok(())
}
```

### 非同期タスク構成

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│  UI Events   │────▶│              │     │   Gateway    │
│   Handler    │     │  Event Queue │◀────│   Handler    │
└──────────────┘     │   (mpsc)     │     └──────────────┘
                     │              │
┌──────────────┐     │              │     ┌──────────────┐
│ Tick Timer   │────▶│              │     │   Command    │
│   (100ms)    │     └──────┬───────┘     │   Executor   │
└──────────────┘            │             └──────────────┘
                            │                     ▲
                            ▼                     │
                     ┌──────────────┐             │
                     │  Main Loop   │─────────────┘
                     │  (UI Render  │
                     │  + Update)   │
                     └──────────────┘
```

---

## エラーハンドリング

### エラー型

```rust
use anyhow::{Context, Result};

// アプリケーションエラー
pub type AppResult<T> = Result<T, anyhow::Error>;
```

### エラー処理方針

1. **致命的エラー**
   - 認証失敗 → プログラム終了
   - Gateway接続失敗 → リトライ後、終了

2. **回復可能エラー**
   - API リクエスト失敗 → エラーメッセージ表示
   - メッセージ送信失敗 → ユーザーに通知

3. **ログ記録**
   - `eprintln!` でエラーログ出力
   - 将来的にファイルログ実装

---

## 設定管理

### 環境変数

```bash
# Discord Bot Token（必須）
export DISCORD_TOKEN="your_bot_token_here"

# オプション
export DISCORD_GUILD_ID="guild_id"  # 対象ギルドID
```

### 設定ファイル（将来実装）

```toml
# config.toml
[discord]
token = "..."
guild_id = "..."

[ui]
theme = "dark"
show_timestamps = true
```

---

## セキュリティ考慮事項

### トークン管理

1. **環境変数から読み込み**
   ```rust
   let token = std::env::var("DISCORD_TOKEN")
       .context("DISCORD_TOKEN environment variable not set")?;
   ```

2. **ハードコード禁止**
   - ソースコードに直接記述しない
   - `.gitignore` に設定ファイルを追加

3. **メモリ保護**
   - トークンをログ出力しない
   - エラーメッセージにトークンを含めない

### TOS違反の警告

**重要**: このプロジェクトはBot APIを使用しますが、ユーザートークンを使用するとDiscord TOS違反となります。

- **Bot Token使用**: TOS準拠
- **User Token使用**: TOS違反（アカウント停止リスク）

学習目的のプロジェクトであることを明記し、実用での使用は推奨しない。

---

## 実装ステップ

### Phase 1: 基礎実装（1-2週間）

1. **プロジェクトセットアップ**
   - [x] Cargo.toml依存関係設定
   - [ ] モジュール構造作成
   - [ ] データモデル定義

2. **Discord REST API**
   - [ ] DiscordRestClient実装
   - [ ] チャンネル取得
   - [ ] メッセージ取得/送信

3. **基本TUI**
   - [ ] レイアウト実装
   - [ ] チャンネルリスト表示
   - [ ] メッセージリスト表示
   - [ ] 入力フィールド

4. **統合**
   - [ ] メインループ実装
   - [ ] イベントハンドリング
   - [ ] 状態管理

### Phase 2: Gateway実装（1週間）

1. **WebSocket接続**
   - [ ] Gateway接続フロー
   - [ ] Identify実装
   - [ ] ハートビート処理

2. **イベント受信**
   - [ ] MESSAGE_CREATE
   - [ ] MESSAGE_UPDATE/DELETE
   - [ ] リアルタイム表示

### Phase 3: 機能拡張（1週間）

1. **DM対応**
   - [ ] DMチャンネル取得
   - [ ] DM送信

2. **UI改善**
   - [ ] スクロール機能
   - [ ] タイムスタンプ整形
   - [ ] エラー表示

### Phase 4: 安定化（1週間）

1. **エラーハンドリング**
   - [ ] レート制限対応
   - [ ] 再接続ロジック
   - [ ] エラー通知

2. **テスト**
   - [ ] 動作確認
   - [ ] バグ修正

---

## 参考リソース

### Discord API

- [Discord Developer Documentation](https://discord.com/developers/docs)
- [Discord Gateway](https://discord.com/developers/docs/topics/gateway)
- [Discord REST API](https://discord.com/developers/docs/resources/channel)

### Rust TUI

- [Ratatui Documentation](https://ratatui.rs/)
- [Ratatui GitHub](https://github.com/ratatui-org/ratatui)
- [Ratatui Examples](https://github.com/ratatui-org/ratatui/tree/main/examples)

### 非同期Rust

- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Async Book](https://rust-lang.github.io/async-book/)

### 参考実装

- [disrust](https://github.com/DvorakDwarf/disrust) - Rust製Discord TUI（アーカイブ済み）
- Ratatui async-template

---

## まとめ

この設計書に基づき、以下の順序で実装を進める：

1. データモデルと基礎構造
2. REST API実装とテスト
3. 基本TUIとイベントループ
4. Gateway統合
5. 機能拡張と安定化

各フェーズで動作確認を行いながら、段階的に機能を追加していく。
