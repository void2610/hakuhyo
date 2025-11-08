use serde::{Deserialize, Serialize};

/// ユーザー情報
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub discriminator: String,
    #[serde(default)]
    pub avatar: Option<String>,
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
    pub recipients: Option<Vec<User>>, // DM用
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
            0 => "# ",  // テキストチャンネル
            1 => "@ ",  // DM
            2 => "🔊 ", // ボイスチャンネル
            _ => "? ",
        }
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

/// Ready イベントのデータ部分
#[derive(Debug, Clone, Deserialize)]
pub struct ReadyData {
    #[allow(dead_code)]
    pub v: u8,
    pub user: User,
    #[allow(dead_code)]
    pub guilds: Vec<UnavailableGuild>,
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UnavailableGuild {
    #[allow(dead_code)]
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub unavailable: Option<bool>,
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
    pub const HELLO: u8 = 10;
    pub const HEARTBEAT_ACK: u8 = 11;
}
