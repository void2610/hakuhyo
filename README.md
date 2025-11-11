# Hakuhyo (薄氷)

Rustで実装された軽量なDiscord TUIクライアント

## 特徴

- **シンプル**: 基本的なメッセージ送受信機能に特化
- **軽量**: TUIベースで低リソース消費
- **直接実装**: Discord用ライブラリを使わず、REST APIとWebSocket Gatewayを直接実装

## 機能

- ✅ チャンネル/DM一覧表示
- ✅ メッセージ表示（テキストのみ）
- ✅ メッセージ送信
- ✅ リアルタイムメッセージ受信

## 未実装機能

- アイコン・画像プレビュー
- 添付ファイル送信
- メンション・リアクション
- メッセージ編集・削除
- スレッド機能

## 必要要件

- Rust 1.70以上
- Discord Bot Token

## インストール

```bash
git clone https://github.com/yourusername/hakuhyo.git
cd hakuhyo
cargo build --release
```

## 使い方

### 1. Discord Bot Tokenの取得

1. [Discord Developer Portal](https://discord.com/developers/applications) にアクセス
2. "New Application" をクリック
3. アプリケーション名を入力
4. 左メニューから "Bot" を選択
5. "Add Bot" をクリック
6. "Token" セクションで "Copy" をクリックしてトークンをコピー
7. **重要**: "Privileged Gateway Intents" セクションで以下を有効化:
   - `MESSAGE CONTENT INTENT` ✅
   - `SERVER MEMBERS INTENT` （オプション）

### 2. ボットをサーバーに招待

1. Discord Developer Portal で左メニューから "OAuth2" → "URL Generator" を選択
2. "SCOPES" で `bot` を選択
3. "BOT PERMISSIONS" で最小限必要な権限を選択:
   - `View Channels` ✅
   - `Send Messages` ✅
   - `Read Message History` ✅
4. 生成されたURLをコピーしてブラウザで開く
5. ボットを招待するサーバーを選択

### 3. 環境変数の設定

```bash
export DISCORD_TOKEN="your_bot_token_here"
```

**注意**: トークンは `Bot ` プレフィックス**なし**で設定してください。
例: `export DISCORD_TOKEN="MTIzNDU2Nzg5MDEyMzQ1Njc4.ABCDEF.xyz..."`

### 4. 実行

```bash
cargo run --release
```

または、ビルド済みバイナリを実行：

```bash
./target/release/hakuhyo
```

## キーバインド

### Normalモード

| キー | 動作 |
|------|------|
| `↑` / `k` | 上のチャンネルを選択 |
| `↓` / `j` | 下のチャンネルを選択 |
| `Enter` | チャンネル選択確定・メッセージ読み込み |
| `i` | 入力モードに切り替え |
| `q` / `Ctrl+C` | 終了 |

### Editingモード

| キー | 動作 |
|------|------|
| `Esc` | Normalモードに戻る |
| `Enter` | メッセージ送信 |
| `Backspace` | 文字削除 |
| 文字キー | 文字入力 |

## プロジェクト構造

```
hakuhyo/
├── Cargo.toml
├── README.md
├── docs/
│   └── DESIGN.md          # 詳細設計書
└── src/
    ├── main.rs            # エントリーポイント、メインループ
    ├── app.rs             # アプリケーション状態管理
    ├── ui.rs              # TUI描画ロジック
    ├── events.rs          # イベント定義
    └── discord/
        ├── mod.rs         # モジュール宣言
        ├── models.rs      # Discord データモデル
        ├── rest.rs        # REST API実装
        └── gateway.rs     # WebSocket Gateway実装
```

## アーキテクチャ

Hakuhyoは **The Elm Architecture (TEA)** パターンに基づいて設計されています：

- **Model**: `AppState` - アプリケーション全体の状態
- **Update**: `app::update()` - イベントを受け取り状態を更新
- **View**: `ui::render()` - 現在の状態を元にTUIを描画

詳細は [`docs/DESIGN.md`](docs/DESIGN.md) を参照してください。

## 技術スタック

- **TUI**: Ratatui + Crossterm
- **非同期**: Tokio
- **HTTP**: Reqwest
- **WebSocket**: tokio-tungstenite
- **JSON**: Serde + serde_json

## 注意事項

### TOS（利用規約）について

このプロジェクトは **Bot API** を使用することを前提としています。

- ✅ **Bot Token使用**: Discord TOS準拠
- ❌ **User Token使用**: Discord TOS違反（アカウント停止のリスク）

**学習目的のプロジェクトです。** 実用での使用は推奨しません。

### セキュリティ

- トークンは環境変数で管理
- `.gitignore` に設定ファイルを追加
- ハードコード厳禁

## トラブルシューティング

### ビルドエラー

```bash
cargo clean
cargo build
```

### Gateway接続エラー: "Gateway connection closed"

このエラーが出る場合、以下を確認してください：

#### 1. トークンの確認

```bash
# トークンが正しく設定されているか確認
echo $DISCORD_TOKEN

# トークンに "Bot " プレフィックスが含まれていないか確認
# ❌ 間違い: export DISCORD_TOKEN="Bot MTIzNDU2..."
# ✅ 正しい: export DISCORD_TOKEN="MTIzNDU2..."
```

#### 2. インテントの有効化

[Discord Developer Portal](https://discord.com/developers/applications) で：
- `MESSAGE CONTENT INTENT` が有効 ✅
- Bot設定で "Reset Token" していないか確認

#### 3. ボットの招待

- ボットが少なくとも1つのサーバーに参加しているか確認
- サーバーでボットがオンライン状態になっているか確認

#### 4. 詳細ログの確認

実行時のログで以下を確認：
```
Received Hello, heartbeat interval: 41250ms  ← OK
Sending Identify with intents: 33281        ← インテント番号を確認
Received: {"op":0,"t":"READY"...            ← READY イベントが来るか
```

`READY` イベントが来ない場合：
- トークンが無効
- インテントが未承認
- Bot設定に問題がある可能性

### メッセージが表示されない

1. `MESSAGE CONTENT INTENT` が有効化されているか確認
2. ボットに必要な権限があるか確認:
   - `View Channels`
   - `Read Message History`
3. チャンネルがボットから見えるか確認（プライベートチャンネルの場合、権限が必要）

## ライセンス

MIT License

## 貢献

Issue や Pull Request を歓迎します。

## 参考リソース

- [Discord Developer Documentation](https://discord.com/developers/docs)
- [Ratatui](https://ratatui.rs/)
- [disrust](https://github.com/DvorakDwarf/disrust) - 参考実装
