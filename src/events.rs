use crate::discord::{Channel, Guild, Message};
use crossterm::event::KeyCode;

/// アプリケーションイベント
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
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
    /// スレッド作成 / 更新（フォーラム投稿含む）
    ThreadUpsert(Channel),
    /// スレッド削除 / アーカイブ
    ThreadDelete { id: String },
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
    /// 過去のメッセージを追加で読み込み完了
    OlderMessagesLoaded {
        channel_id: String,
        messages: Vec<Message>,
    },
    /// チャンネルのメッセージ取得が失敗 (権限なし等)
    MessagesLoadFailed { channel_id: String },
    /// メッセージリストを行単位でスクロール (正: 古い側へ / 負: 新しい側へ)
    ScrollMessages(i32),
    /// 画像添付ファイルのデコード完了 (DynamicImage は重いので Box で包む)
    AttachmentImageLoaded {
        attachment_id: String,
        image: Box<image::DynamicImage>,
    },
    /// 画像添付ファイルのダウンロード/デコード失敗 (再試行可能にするためロック解除用)
    AttachmentImageFailed { attachment_id: String },

    // システムイベント
    /// 定期的な描画更新
    Tick,
    /// アプリケーション終了
    Quit,
}
