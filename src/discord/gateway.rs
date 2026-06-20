use super::models::{self, *};
use anyhow::{Context, Result};
use futures::stream::{SplitSink, SplitStream};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsWrite = SplitSink<WsStream, WsMessage>;
type WsRead = SplitStream<WsStream>;

/// 切断後の再接続方針
enum ConnectionOutcome {
    /// 同一セッションで再接続（RESUME を試みる）
    Reconnect,
    /// セッション無効。resumable=false なら再 IDENTIFY
    InvalidSession { resumable: bool },
}

/// メッセージ処理結果
enum MessageResult {
    Event(GatewayEvent),
    Reconnect,
    InvalidSession { resumable: bool },
    Ignore,
}

/// Gateway クライアント
pub struct GatewayClient {
    token: String,
    gateway_url: String,
    #[allow(dead_code)]
    intents: u32,
    last_sequence: Arc<RwLock<Option<u64>>>,
    session_id: Option<String>,
    resume_gateway_url: Option<String>,
}

impl GatewayClient {
    /// Gateway クライアントを初期化（実際の接続は run() 内で確立）
    pub fn new(token: String, gateway_url: String) -> Self {
        // インテント設定（ギルド、メッセージ、DM、メッセージ内容）
        let intents = intents::GUILDS
            | intents::GUILD_MESSAGES
            | intents::DIRECT_MESSAGES
            | intents::MESSAGE_CONTENT;

        Self {
            token,
            gateway_url,
            intents,
            last_sequence: Arc::new(RwLock::new(None)),
            session_id: None,
            resume_gateway_url: None,
        }
    }

    /// Gateway イベントループを開始（切断時は自動で再接続・RESUME）
    pub async fn run<F>(mut self, mut event_handler: F) -> Result<()>
    where
        F: FnMut(GatewayEvent) + Send + 'static,
    {
        loop {
            // 有効なセッションがあれば resume_gateway_url で RESUME を試みる
            let (url, resume) = match (&self.resume_gateway_url, &self.session_id) {
                (Some(u), Some(_)) => (u.clone(), true),
                _ => (self.gateway_url.clone(), false),
            };

            let ws_stream = match Self::establish(&url).await {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to connect to Gateway: {:?}, retrying in 5s", e);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };

            match self.connection_loop(ws_stream, resume, &mut event_handler).await {
                ConnectionOutcome::Reconnect => {
                    log::warn!("Gateway disconnected, reconnecting...");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                ConnectionOutcome::InvalidSession { resumable } => {
                    if !resumable {
                        // セッションを破棄して再 IDENTIFY
                        log::warn!("Session invalidated, re-identifying with a new session");
                        self.session_id = None;
                        self.resume_gateway_url = None;
                        *self.last_sequence.write().await = None;
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    /// WebSocket 接続を1つ確立
    async fn establish(url: &str) -> Result<WsStream> {
        let ws_url = format!("{}/?v=10&encoding=json", url);
        log::info!("Connecting to Gateway: {}", ws_url);

        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .context("Failed to connect to Gateway")?;

        log::info!("Connected to Gateway");
        Ok(ws_stream)
    }

    /// 1接続分のイベントループ。切断時に再接続方針を返す
    async fn connection_loop<F>(
        &mut self,
        ws_stream: WsStream,
        resume: bool,
        event_handler: &mut F,
    ) -> ConnectionOutcome
    where
        F: FnMut(GatewayEvent) + Send + 'static,
    {
        let (mut write, mut read) = ws_stream.split();

        // Hello を受信してハートビート間隔を取得
        let heartbeat_interval = match Self::wait_for_hello(&mut read).await {
            Ok(i) => i,
            Err(e) => {
                log::error!("Failed to receive Hello: {:?}", e);
                return ConnectionOutcome::Reconnect;
            }
        };
        log::info!("Received Hello, heartbeat interval: {}ms", heartbeat_interval);

        // RESUME 可能なら RESUME、そうでなければ IDENTIFY
        let send_result = if resume {
            let seq = *self.last_sequence.read().await;
            let session_id = self.session_id.clone().unwrap_or_default();
            log::info!("Resuming session {} (seq={:?})", session_id, seq);
            Self::send_resume(&mut write, &self.token, &session_id, seq).await
        } else {
            log::info!("Sending Identify");
            Self::send_identify(&mut write, &self.token).await
        };
        if let Err(e) = send_result {
            log::error!("Failed to send Identify/Resume: {:?}", e);
            return ConnectionOutcome::Reconnect;
        }

        // ハートビートタスクを開始（write を move）
        let hb_seq = self.last_sequence.clone();
        let hb_handle = tokio::spawn(async move {
            Self::heartbeat_loop(&mut write, heartbeat_interval, hb_seq).await;
        });

        // イベント受信ループ
        let outcome = loop {
            match read.next().await {
                Some(Ok(WsMessage::Text(text))) => {
                    log::debug!("Received: {}", text);
                    match Self::handle_message(&text, self).await {
                        MessageResult::Event(event) => event_handler(event),
                        MessageResult::Reconnect => break ConnectionOutcome::Reconnect,
                        MessageResult::InvalidSession { resumable } => {
                            break ConnectionOutcome::InvalidSession { resumable }
                        }
                        MessageResult::Ignore => {}
                    }
                }
                Some(Ok(WsMessage::Close(frame))) => {
                    log::warn!("Gateway connection closed: {:?}", frame);
                    break ConnectionOutcome::Reconnect;
                }
                Some(Err(e)) => {
                    log::error!("WebSocket error: {}", e);
                    break ConnectionOutcome::Reconnect;
                }
                None => {
                    log::warn!("Gateway stream ended");
                    break ConnectionOutcome::Reconnect;
                }
                _ => {}
            }
        };

        // ハートビートタスクを停止
        hb_handle.abort();
        outcome
    }

    /// Hello メッセージを待機
    async fn wait_for_hello(read: &mut WsRead) -> Result<u64> {
        while let Some(msg) = read.next().await {
            if let Ok(WsMessage::Text(text)) = msg {
                let payload: GatewayPayload = serde_json::from_str(&text)
                    .context("Failed to parse Hello payload")?;

                if payload.op == opcodes::HELLO {
                    let data: HelloData = serde_json::from_value(
                        payload.d.context("Hello payload missing data")?,
                    )
                    .context("Failed to parse Hello data")?;

                    return Ok(data.heartbeat_interval);
                }
            }
        }

        anyhow::bail!("Failed to receive Hello from Gateway")
    }

    /// Identify を送信
    async fn send_identify(write: &mut WsWrite, token: &str) -> Result<()> {
        // ユーザーアカウント認証用の詳細なproperties
        // 実際のDiscordクライアントを模倣
        let identify_payload = json!({
            "op": opcodes::IDENTIFY,
            "d": {
                "token": token,
                "capabilities": 16381,  // ユーザークライアントの機能フラグ
                "properties": {
                    "os": "Mac OS X",
                    "browser": "Chrome",
                    "device": "",
                    "system_locale": "ja-JP",
                    "browser_user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
                    "browser_version": "120.0.0.0",
                    "os_version": "10.15.7",
                    "referrer": "",
                    "referring_domain": "",
                    "referrer_current": "",
                    "referring_domain_current": "",
                    "release_channel": "stable",
                    "client_build_number": 261053,
                    "client_event_source": serde_json::Value::Null
                },
                "presence": {
                    "status": "online",
                    "since": 0,
                    "activities": [],
                    "afk": false
                },
                "compress": false,
                "client_state": {
                    "guild_versions": {},
                    "highest_last_message_id": "0",
                    "read_state_version": 0,
                    "user_guild_settings_version": -1,
                    "user_settings_version": -1,
                    "private_channels_version": "0",
                    "api_code_version": 0
                }
            }
        });

        let payload_text = serde_json::to_string(&identify_payload)?;
        log::debug!("Identify payload: {}", payload_text);
        write
            .send(WsMessage::Text(payload_text))
            .await
            .context("Failed to send Identify")?;

        Ok(())
    }

    /// Resume を送信（切断したセッションの再開）
    async fn send_resume(
        write: &mut WsWrite,
        token: &str,
        session_id: &str,
        seq: Option<u64>,
    ) -> Result<()> {
        let resume_payload = json!({
            "op": opcodes::RESUME,
            "d": {
                "token": token,
                "session_id": session_id,
                "seq": seq
            }
        });

        let payload_text = serde_json::to_string(&resume_payload)?;
        write
            .send(WsMessage::Text(payload_text))
            .await
            .context("Failed to send Resume")?;

        Ok(())
    }

    /// ハートビートループ
    async fn heartbeat_loop(
        write: &mut WsWrite,
        interval_ms: u64,
        last_sequence: Arc<RwLock<Option<u64>>>,
    ) {
        let mut ticker = interval(Duration::from_millis(interval_ms));

        loop {
            ticker.tick().await;

            let seq = *last_sequence.read().await;
            // ハートビートペイロードを直接構築（s と t フィールドを含めない）
            let heartbeat = json!({
                "op": opcodes::HEARTBEAT,
                "d": seq
            });

            if let Ok(payload_text) = serde_json::to_string(&heartbeat) {
                if write.send(WsMessage::Text(payload_text)).await.is_err() {
                    log::error!("Failed to send heartbeat");
                    break;
                }
            }
        }
    }

    /// メッセージを処理
    async fn handle_message(text: &str, client: &mut GatewayClient) -> MessageResult {
        let payload: GatewayPayload = match serde_json::from_str(text) {
            Ok(p) => p,
            Err(_) => return MessageResult::Ignore,
        };

        // シーケンス番号を更新
        if let Some(seq) = payload.s {
            *client.last_sequence.write().await = Some(seq);
        }

        match payload.op {
            opcodes::DISPATCH => Self::handle_dispatch(payload, client),
            opcodes::RECONNECT => {
                // サーバーから再接続要求（RESUME 可能）
                log::info!("Gateway requested reconnect (op 7)");
                MessageResult::Reconnect
            }
            opcodes::INVALID_SESSION => {
                // d が true なら RESUME 可能、false なら再 IDENTIFY が必要
                let resumable = payload.d.and_then(|v| v.as_bool()).unwrap_or(false);
                log::warn!("Invalid session (op 9), resumable={}", resumable);
                MessageResult::InvalidSession { resumable }
            }
            opcodes::HEARTBEAT_ACK => MessageResult::Ignore,
            _ => MessageResult::Ignore,
        }
    }

    /// DISPATCH イベントを処理
    fn handle_dispatch(payload: GatewayPayload, client: &mut GatewayClient) -> MessageResult {
        let event_type = match payload.t.as_deref() {
            Some(t) => t,
            None => return MessageResult::Ignore,
        };
        let data = match payload.d {
            Some(d) => d,
            None => return MessageResult::Ignore,
        };

        match event_type {
            "READY" => {
                // ユーザーアカウント認証の場合、READY イベントに全てのギルド情報が含まれる
                // ギルド・チャンネルの実際の抽出と登録は app::update() 側で行う
                if let Some(session_id) = data.get("session_id").and_then(|v| v.as_str()) {
                    client.session_id = Some(session_id.to_string());
                }
                // RESUME 用の専用 Gateway URL を保存
                client.resume_gateway_url = data
                    .get("resume_gateway_url")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if let Some(user) = data.get("user").and_then(|v| v.get("username")).and_then(|v| v.as_str()) {
                    log::info!("Gateway Ready! User: {}", user);
                }
                if let Some(guilds_array) = data.get("guilds").and_then(|v| v.as_array()) {
                    log::info!("READY event contains {} guilds", guilds_array.len());
                }
                if let Some(private_channels) = data.get("private_channels").and_then(|v| v.as_array()) {
                    log::info!("READY event contains {} private_channels", private_channels.len());
                } else {
                    log::warn!("READY event does NOT contain private_channels field");
                }

                MessageResult::Event(GatewayEvent::Ready(data))
            }
            "RESUMED" => {
                log::info!("Gateway session resumed successfully");
                MessageResult::Ignore
            }
            "GUILD_CREATE" => {
                // ギルド情報を抽出
                let result = (|| {
                    let guild_id = data.get("id")?.as_str()?.to_string();
                    let guild_name = data.get("name")?.as_str()?.to_string();
                    let owner_id = data.get("owner_id")?.as_str()?.to_string();
                    let icon = data.get("icon").and_then(|v| v.as_str()).map(|s| s.to_string());

                    let guild = models::Guild {
                        id: guild_id.clone(),
                        name: guild_name,
                        icon,
                        owner_id,
                    };

                    log::info!("GUILD_CREATE: {} ({})", guild.name, guild.id);

                    // チャンネル情報を抽出
                    let channels = data.get("channels")?.as_array()?;
                    let mut channel_list = Vec::new();

                    for channel_data in channels {
                        if let Ok(mut channel) = serde_json::from_value::<models::Channel>(channel_data.clone()) {
                            // GUILD_CREATEのチャンネルにはguild_idが含まれない場合がある
                            if channel.guild_id.is_none() {
                                channel.guild_id = Some(guild_id.clone());
                            }

                            // テキストチャンネル（type 0）のみ追加
                            if channel.channel_type == 0 {
                                channel_list.push(channel);
                            }
                        }
                    }

                    log::info!("GUILD_CREATE: loaded {} text channels for guild: {}", channel_list.len(), guild.name);
                    Some(GatewayEvent::GuildCreate { guild, channels: channel_list })
                })();

                match result {
                    Some(event) => MessageResult::Event(event),
                    None => MessageResult::Ignore,
                }
            }
            "MESSAGE_CREATE" => match serde_json::from_value::<models::Message>(data) {
                Ok(message) => MessageResult::Event(GatewayEvent::MessageCreate(message)),
                Err(_) => MessageResult::Ignore,
            },
            "MESSAGE_UPDATE" => match serde_json::from_value::<models::Message>(data) {
                Ok(message) => MessageResult::Event(GatewayEvent::MessageUpdate(message)),
                Err(_) => MessageResult::Ignore,
            },
            "MESSAGE_DELETE" => {
                let result = (|| {
                    let id = data.get("id")?.as_str()?.to_string();
                    let channel_id = data.get("channel_id")?.as_str()?.to_string();
                    Some(GatewayEvent::MessageDelete { id, channel_id })
                })();
                match result {
                    Some(event) => MessageResult::Event(event),
                    None => MessageResult::Ignore,
                }
            }
            _ => MessageResult::Ignore,
        }
    }
}

/// Gateway イベント
#[derive(Debug, Clone)]
pub enum GatewayEvent {
    Ready(serde_json::Value),  // READY イベント全体（ギルド情報含む）
    GuildCreate { guild: models::Guild, channels: Vec<models::Channel> },
    MessageCreate(models::Message),
    MessageUpdate(models::Message),
    MessageDelete { id: String, channel_id: String },
}
