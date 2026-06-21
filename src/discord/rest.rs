use super::models::*;
use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

/// `get_messages` 用のエラー型。HTTP status を取り出して呼び出し側で
/// 一時エラーと永続エラーを区別できるようにする。
#[derive(Debug)]
pub enum RestError {
    /// HTTP 応答エラー (4xx / 5xx)
    Http { status: u16, body: String },
    /// 送信失敗やネットワークエラー等
    Network(anyhow::Error),
}

impl std::fmt::Display for RestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestError::Http { status, body } => {
                write!(f, "HTTP {} - {}", status, body)
            }
            RestError::Network(e) => write!(f, "network error: {}", e),
        }
    }
}

impl std::error::Error for RestError {}

const API_BASE: &str = "https://discord.com/api/v10";

/// Discord REST API クライアント
#[derive(Clone)]
pub struct DiscordRestClient {
    client: Client,
    token: String,
}

impl DiscordRestClient {
    /// 新しいREST APIクライアントを作成
    pub fn new(token: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to create HTTP client");

        Self { client, token }
    }

    /// チャンネルのメッセージを取得。失敗時は HTTP status を含む構造化エラーを返す
    /// (呼び出し側で 4xx/5xx/ネットワークの違いを判別するため)。
    /// `before` を指定すると、その message_id より古いものを返す
    pub async fn get_messages(
        &self,
        channel_id: &str,
        limit: u8,
        before: Option<&str>,
    ) -> std::result::Result<Vec<Message>, RestError> {
        let mut url = format!(
            "{}/channels/{}/messages?limit={}",
            API_BASE,
            channel_id,
            limit.min(100)
        );
        if let Some(before_id) = before {
            url.push_str(&format!("&before={}", before_id));
        }
        // レート制限対策: 最小間隔を設ける
        tokio::time::sleep(Duration::from_millis(20)).await;
        let response = self
            .client
            .get(&url)
            .header("Authorization", self.token.clone())
            .header("User-Agent", "Hakuhyo/1.0")
            .send()
            .await
            .map_err(|e| RestError::Network(anyhow::Error::new(e)))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(RestError::Http {
                status: status.as_u16(),
                body,
            });
        }
        response
            .json::<Vec<Message>>()
            .await
            .map_err(|e| RestError::Network(anyhow::Error::new(e).context("Failed to parse messages JSON")))
    }

    /// メッセージを送信
    pub async fn send_message(&self, channel_id: &str, content: &str) -> Result<Message> {
        let url = format!("{}/channels/{}/messages", API_BASE, channel_id);
        let payload = CreateMessagePayload {
            content: content.to_string(),
        };
        self.post(&url, &payload).await
    }

    /// メッセージを既読としてマークする (ユーザーアカウント用)
    /// レスポンスはトークン入りの JSON や空 body のことがあるため、デコードは行わない
    pub async fn ack_message(&self, channel_id: &str, message_id: &str) -> Result<()> {
        let url = format!(
            "{}/channels/{}/messages/{}/ack",
            API_BASE, channel_id, message_id
        );
        let payload = serde_json::json!({ "token": serde_json::Value::Null });
        tokio::time::sleep(Duration::from_millis(20)).await;
        let response = self
            .client
            .post(&url)
            .header("Authorization", self.token.clone())
            .header("User-Agent", "Hakuhyo/1.0")
            .json(&payload)
            .send()
            .await
            .context("Failed to send ack request")?;
        let status = response.status();
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Ack failed with status {}: {}", status, text);
        }
        Ok(())
    }

    /// Gateway URLを取得
    pub async fn get_gateway_url(&self) -> Result<String> {
        // ユーザーアカウント認証対応: /gateway エンドポイントを使用
        let url = format!("{}/gateway", API_BASE);
        let response: GatewayResponse = self.get(&url).await?;
        Ok(response.url)
    }

    /// GETリクエストを送信
    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        // レート制限対策: 最小間隔を設ける
        tokio::time::sleep(Duration::from_millis(20)).await;

        // トークンをそのまま使用（ユーザーアカウント認証対応）
        let auth_header = self.token.clone();

        let response = self
            .client
            .get(url)
            .header("Authorization", auth_header)
            .header("User-Agent", "Hakuhyo/1.0")
            .send()
            .await
            .context("Failed to send GET request")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Request failed with status {}: {}", status, error_text);
        }

        let data = response
            .json::<T>()
            .await
            .context("Failed to parse JSON response")?;

        Ok(data)
    }

    /// POSTリクエストを送信
    async fn post<T: serde::Serialize, R: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        payload: &T,
    ) -> Result<R> {
        // レート制限対策: 最小間隔を設ける
        tokio::time::sleep(Duration::from_millis(20)).await;

        // トークンをそのまま使用（ユーザーアカウント認証対応）
        let auth_header = self.token.clone();

        let response = self
            .client
            .post(url)
            .header("Authorization", auth_header)
            .header("User-Agent", "Hakuhyo/1.0")
            .json(payload)
            .send()
            .await
            .context("Failed to send POST request")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Request failed with status {}: {}", status, error_text);
        }

        let data = response
            .json::<R>()
            .await
            .context("Failed to parse JSON response")?;

        Ok(data)
    }
}
