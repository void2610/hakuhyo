use serde::{Deserialize, Serialize};

/// ユーザー情報
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    #[serde(default)]
    pub avatar: Option<String>,
    /// グローバル表示名 (旧 discriminator 廃止後の新表示名)
    #[serde(default)]
    pub global_name: Option<String>,
}

/// 添付ファイル情報
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Attachment {
    pub id: String,
    pub filename: String,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
}

impl Attachment {
    /// 添付ファイルの表示用テキストを取得
    pub fn display_text(&self) -> String {
        if let Some(content_type) = &self.content_type {
            if content_type.starts_with("image/") {
                format!("[Image: {}]", self.filename)
            } else if content_type.starts_with("video/") {
                format!("[Video: {}]", self.filename)
            } else if content_type.starts_with("audio/") {
                format!("[Audio: {}]", self.filename)
            } else {
                format!("[File: {}]", self.filename)
            }
        } else {
            format!("[File: {}]", self.filename)
        }
    }
}

/// メッセージ情報
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    pub id: String,
    pub channel_id: String,
    pub author: User,
    pub content: String,
    pub timestamp: String,
    #[serde(default)]
    pub edited_timestamp: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    /// ギルドメンバー情報 (サーバー内発言時にニックネームを含む)
    #[serde(default)]
    pub member: Option<MessageMember>,
}

/// メッセージに付与される partial guild member
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MessageMember {
    #[serde(default)]
    pub nick: Option<String>,
}

/// READY イベント内 read_state エントリ (チャンネル毎の既読状態)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadStateEntry {
    pub id: String, // channel_id
    #[serde(default)]
    pub last_message_id: Option<String>,
    #[serde(default)]
    pub mention_count: u32,
}

/// READY イベント内 user_guild_settings エントリ (サーバー単位の通知設定)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UserGuildSettingsEntry {
    pub guild_id: Option<String>, // null は DM 設定
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub channel_overrides: Vec<ChannelOverride>,
}

/// チャンネル単位の通知 override
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelOverride {
    pub channel_id: String,
    #[serde(default)]
    pub muted: bool,
}

impl Message {
    /// 表示用の作者名を取得 (サーバーニックネーム → global_name → username の優先順)
    pub fn author_display_name(&self) -> &str {
        if let Some(nick) = self.member.as_ref().and_then(|m| m.nick.as_deref()) {
            if !nick.is_empty() {
                return nick;
            }
        }
        if let Some(global) = self.author.global_name.as_deref() {
            if !global.is_empty() {
                return global;
            }
        }
        &self.author.username
    }
}

/// チャンネル情報
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Channel {
    pub id: String,
    #[serde(rename = "type")]
    pub channel_type: u8,
    #[serde(default)]
    pub guild_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub recipients: Option<Vec<User>>, // DM用（完全なユーザー情報）
    #[serde(default)]
    pub recipient_ids: Option<Vec<String>>, // DM用（ユーザーIDのみ、READYイベントで使用）
    #[serde(default)]
    pub parent_id: Option<String>, // スレッドの親チャンネル / カテゴリ
    #[serde(default)]
    pub last_message_id: Option<String>, // 直近メッセージ ID (未読判定用)
}

impl Channel {
    /// チャンネルの表示名を取得
    pub fn display_name(&self) -> String {
        if let Some(name) = &self.name {
            name.clone()
        } else if let Some(recipients) = &self.recipients {
            // DM の場合は相手のユーザー名を使用
            recipients
                .first()
                .map(|u| u.username.clone())
                .unwrap_or_else(|| "Unknown".to_string())
        } else {
            "Unknown".to_string()
        }
    }

    /// チャンネルタイプのプレフィックスを取得
    pub fn type_prefix(&self) -> &str {
        match self.channel_type {
            0 => "# ",        // テキストチャンネル
            1 => "@ ",        // DM
            2 => "🔊 ",       // ボイスチャンネル
            3 => "@@ ",       // グループ DM
            5 => "📢 ",       // アナウンスチャンネル
            10 | 11 | 12 => "🧵 ", // スレッド
            15 => "📋 ",      // フォーラム
            16 => "🖼️ ",     // メディアチャンネル
            _ => "? ",
        }
    }

    /// メッセージのやり取りが可能なチャンネルかどうか
    /// (DM/グループDM/テキスト/アナウンス/各種スレッド)
    pub fn is_messageable(&self) -> bool {
        matches!(self.channel_type, 0 | 1 | 3 | 5 | 10 | 11 | 12)
    }
}

/// ギルド（サーバー）情報
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Guild {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    pub owner_id: String,
}

/// Gateway URL レスポンス
#[derive(Debug, Deserialize)]
pub struct GatewayResponse {
    pub url: String,
}

/// Gateway ペイロード
#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayPayload {
    pub op: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub d: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub t: Option<String>,
}

/// Hello ペイロードのデータ部分
#[derive(Debug, Deserialize)]
pub struct HelloData {
    pub heartbeat_interval: u64,
}

/// Identify ペイロードのデータ部分
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct IdentifyData {
    pub token: String,
    pub intents: u32,
    pub properties: IdentifyProperties,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct IdentifyProperties {
    pub os: String,
    pub browser: String,
    pub device: String,
}

/// メッセージ作成リクエストのペイロード
#[derive(Debug, Serialize)]
pub struct CreateMessagePayload {
    pub content: String,
}

/// Gateway インテント定数
pub mod intents {
    pub const GUILDS: u32 = 1 << 0;
    pub const GUILD_MESSAGES: u32 = 1 << 9;
    pub const DIRECT_MESSAGES: u32 = 1 << 12;
    pub const MESSAGE_CONTENT: u32 = 1 << 15;
}

/// Gateway Opcode 定数
pub mod opcodes {
    pub const DISPATCH: u8 = 0;
    pub const HEARTBEAT: u8 = 1;
    pub const IDENTIFY: u8 = 2;
    pub const RESUME: u8 = 6;
    pub const RECONNECT: u8 = 7;
    pub const INVALID_SESSION: u8 = 9;
    pub const HELLO: u8 = 10;
    pub const HEARTBEAT_ACK: u8 = 11;
}
