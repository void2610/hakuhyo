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
- Discordアカウント（モバイルアプリ必須）

## インストール

```bash
git clone https://github.com/yourusername/hakuhyo.git
cd hakuhyo
cargo build --release
```

## 使い方

### 1. 実行

```bash
cargo run --release
```

または、ビルド済みバイナリを実行：

```bash
./target/release/hakuhyo
```

### 2. QRコード認証

初回起動時、または保存されたトークンが無効な場合、QRコードが表示されます：

1. ターミナルに表示されるQRコードをスキャン
2. Discordモバイルアプリで「設定」→「QRコードでログイン」を選択
3. QRコードをスキャンして「はい、ログインします」をタップ
4. 認証完了後、トークンはシステムキーチェーンに自動保存されます

**次回起動時は自動的にログインします。**

## キーバインド

### Normalモード

| キー | 動作 |
|------|------|
| `/` | 検索モードに切り替え |
| `↑` / `k` | 上のチャンネルを選択 |
| `↓` / `j` | 下のチャンネルを選択 |
| `Enter` | チャンネル選択確定・メッセージ読み込み |
| `f` | お気に入りに登録/解除 |
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
├── CLAUDE.md             # Claude Code向けガイド
└── src/
    ├── main.rs           # エントリーポイント、メインループ
    ├── app.rs            # アプリケーション状態管理
    ├── ui.rs             # TUI描画ロジック
    ├── events.rs         # イベント定義
    ├── auth.rs           # QRコード認証
    ├── token_store.rs    # キーチェーン統合
    ├── config.rs         # お気に入り永続化
    └── discord/
        ├── mod.rs        # モジュール宣言
        ├── models.rs     # Discord データモデル
        ├── rest.rs       # REST API実装
        └── gateway.rs    # WebSocket Gateway実装
```

## アーキテクチャ

Hakuhyoは **The Elm Architecture (TEA)** パターンに基づいて設計されています：

- **Model**: `AppState` - アプリケーション全体の状態
- **Update**: `app::update()` - イベントを受け取り状態を更新
- **View**: `ui::render()` - 現在の状態を元にTUIを描画

詳細は [`CLAUDE.md`](CLAUDE.md) を参照してください。

## 技術スタック

- **TUI**: Ratatui + Crossterm
- **非同期**: Tokio
- **HTTP**: Reqwest
- **WebSocket**: tokio-tungstenite
- **JSON**: Serde + serde_json

## 注意事項

### TOS（利用規約）について

このプロジェクトは **ユーザーアカウント認証** を使用します。

- ⚠️ **ユーザーアカウント認証**: Discord利用規約違反の可能性があります
- ⚠️ **アカウント停止リスク**: 使用は自己責任でお願いします

**これは学習目的のプロジェクトです。** 実用での使用は推奨しません。

### セキュリティ

- **保存先**: `~/.config/hakuhyo/token.txt`
- **ファイルパーミッション**: 0600（所有者のみ読み書き可能）

## トラブルシューティング

### ビルドエラー

```bash
cargo clean
cargo build
```

### QRコード認証ができない

1. ターミナルでQRコードが正しく表示されているか確認
2. Discordモバイルアプリで「設定」→「QRコードでログイン」を使用
3. QRコードをスキャンして「はい、ログインします」をタップ

### 保存されたトークンをクリアしたい

```bash
cargo run --release --example clear_token
```

次回起動時に再度QRコード認証が必要になります。

## ライセンス

MIT License

## 貢献

Issue や Pull Request を歓迎します。

## 参考リソース

- [Discord Developer Documentation](https://discord.com/developers/docs)
- [Ratatui](https://ratatui.rs/)
- [disrust](https://github.com/DvorakDwarf/disrust) - 参考実装
