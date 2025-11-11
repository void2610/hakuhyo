use crate::discord::{Channel, Guild, Message};
use crossterm::event::KeyCode;

/// アプリケーションイベント
#[derive(Debug, Clone)]
pub enum AppEvent {
    // UI イベント
    /// キー入力
    KeyPress(KeyCode),
    /// 文字入力（編集モード時）
    #[allow(dead_code)]
    Input(char),

    // Discord イベント（Gateway）
    /// Gateway接続完了（READY イベント全体）
    GatewayReady(serde_json::Value),
    /// ギルド作成（READY後の新規ギルド参加用）
    GuildCreate { guild: Guild, channels: Vec<Channel> },
    /// 新規メッセージ
    MessageCreate(Message),
    /// メッセージ更新
    MessageUpdate(Message),
    /// メッセージ削除
    MessageDelete { id: String, channel_id: String },

    // コマンド完了イベント（REST API の結果）
    /// メッセージ一覧読み込み完了
    MessagesLoaded {
        channel_id: String,
        messages: Vec<Message>,
    },
    /// メッセージ送信完了
    MessageSent(Message),

    // システムイベント
    /// 定期的な描画更新
    Tick,
    /// アプリケーション終了
    Quit,
}
