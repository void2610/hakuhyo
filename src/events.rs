use crate::discord::{Channel, Guild, Message, ReadyData};
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
    /// Gateway接続完了
    GatewayReady(ReadyData),
    /// ギルド作成（チャンネル情報取得）
    GuildCreate(Vec<Channel>),
    /// 新規メッセージ
    MessageCreate(Message),
    /// メッセージ更新
    MessageUpdate(Message),
    /// メッセージ削除
    MessageDelete { id: String, channel_id: String },

    // コマンド完了イベント（REST API の結果）
    /// ギルド情報読み込み完了
    GuildLoaded(Guild),
    /// チャンネル一覧読み込み完了
    ChannelsLoaded(Vec<Channel>),
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
