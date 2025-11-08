use super::models::{self, *};
use anyhow::{Context, Result};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

/// Gateway クライアント
pub struct GatewayClient {
    ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    token: String,
    intents: u32,
    last_sequence: Arc<RwLock<Option<u64>>>,
    session_id: Option<String>,
}

impl GatewayClient {
    /// Gateway に接続
    pub async fn connect(token: String, gateway_url: String) -> Result<Self> {
        // WebSocket URL を構築
        let ws_url = format!("{}/?v=10&encoding=json", gateway_url);

        log::info!("Connecting to Gateway: {}", ws_url);

        // WebSocket接続
        let (ws_stream, _) = connect_async(&ws_url)
            .await
            .context("Failed to connect to Gateway")?;

        log::info!("Connected to Gateway");

        // インテント設定（ギルド、メッセージ、DM、メッセージ内容）
        let intents = intents::GUILDS
            | intents::GUILD_MESSAGES
            | intents::DIRECT_MESSAGES
            | intents::MESSAGE_CONTENT;

        Ok(Self {
            ws_stream,
            token,
            intents,
            last_sequence: Arc::new(RwLock::new(None)),
            session_id: None,
        })
    }

    /// Gateway イベントループを開始
    pub async fn run<F>(mut self, mut event_handler: F) -> Result<()>
    where
        F: FnMut(GatewayEvent) + Send + 'static,
    {
        // Hello メッセージを受信してハートビート間隔を取得
        let heartbeat_interval = self.wait_for_hello().await?;

        log::info!("Received Hello, heartbeat interval: {}ms", heartbeat_interval);

        // Identify を送信
        self.send_identify().await?;

        log::info!("Sent Identify");

        // ハートビートタスクを開始
        let last_seq_clone = self.last_sequence.clone();
        let (mut write, mut read) = self.ws_stream.split();

        tokio::spawn(async move {
            Self::heartbeat_loop(&mut write, heartbeat_interval, last_seq_clone).await;
        });

        // イベント受信ループ
        let mut session_id = self.session_id;
        let last_seq_ref = self.last_sequence.clone();

        while let Some(msg) = read.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    log::debug!("Received: {}", text);
                    if let Some(event) = Self::handle_message(&text, &mut session_id, &last_seq_ref).await {
                        event_handler(event);
                    }
                }
                Ok(WsMessage::Close(frame)) => {
                    log::warn!("Gateway connection closed: {:?}", frame);
                    break;
                }
                Err(e) => {
                    log::error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Hello メッセージを待機
    async fn wait_for_hello(&mut self) -> Result<u64> {
        while let Some(msg) = self.ws_stream.next().await {
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
    async fn send_identify(&mut self) -> Result<()> {
        // トークンに "Bot " プレフィックスがない場合は追加
        let token = if self.token.starts_with("Bot ") {
            self.token.clone()
        } else {
            format!("Bot {}", self.token)
        };

        // Identify ペイロードを直接構築（s と t フィールドを含めない）
        let identify_payload = json!({
            "op": opcodes::IDENTIFY,
            "d": {
                "token": token,
                "intents": self.intents,
                "properties": {
                    "os": std::env::consts::OS,
                    "browser": "hakuhyo",
                    "device": "hakuhyo"
                }
            }
        });

        let payload_text = serde_json::to_string(&identify_payload)?;
        log::info!("Sending Identify with intents: {}", self.intents);
        log::debug!("Identify payload: {}", payload_text);
        self.ws_stream
            .send(WsMessage::Text(payload_text))
            .await
            .context("Failed to send Identify")?;

        Ok(())
    }

    /// ハートビートループ
    async fn heartbeat_loop(
        write: &mut futures::stream::SplitSink<
            WebSocketStream<MaybeTlsStream<TcpStream>>,
            WsMessage,
        >,
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
    async fn handle_message(
        text: &str,
        session_id: &mut Option<String>,
        last_sequence: &Arc<RwLock<Option<u64>>>,
    ) -> Option<GatewayEvent> {
        let payload: GatewayPayload = serde_json::from_str(text).ok()?;

        // シーケンス番号を更新
        if let Some(seq) = payload.s {
            *last_sequence.write().await = Some(seq);
        }

        match payload.op {
            opcodes::DISPATCH => {
                let event_type = payload.t.as_deref()?;
                let data = payload.d?;

                match event_type {
                    "READY" => {
                        let ready_data: ReadyData = serde_json::from_value(data).ok()?;
                        *session_id = Some(ready_data.session_id.clone());
                        log::info!("Gateway Ready! User: {}", ready_data.user.username);
                        Some(GatewayEvent::Ready(ready_data))
                    }
                    "MESSAGE_CREATE" => {
                        let message: models::Message = serde_json::from_value(data).ok()?;
                        Some(GatewayEvent::MessageCreate(message))
                    }
                    "MESSAGE_UPDATE" => {
                        // 簡略化: フル Message をパースして返す
                        let message: models::Message = serde_json::from_value(data).ok()?;
                        Some(GatewayEvent::MessageUpdate(message))
                    }
                    "MESSAGE_DELETE" => {
                        let id = data.get("id")?.as_str()?.to_string();
                        let channel_id = data.get("channel_id")?.as_str()?.to_string();
                        Some(GatewayEvent::MessageDelete { id, channel_id })
                    }
                    _ => {
                        // その他のイベントは無視
                        None
                    }
                }
            }
            opcodes::HEARTBEAT_ACK => {
                // ハートビートACKは特に処理不要
                None
            }
            _ => None,
        }
    }
}

/// Gateway イベント
#[derive(Debug, Clone)]
pub enum GatewayEvent {
    Ready(ReadyData),
    MessageCreate(models::Message),
    MessageUpdate(models::Message),
    MessageDelete { id: String, channel_id: String },
}
