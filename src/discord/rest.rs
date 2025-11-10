use super::models::*;
use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

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

    /// ギルド一覧を取得
    pub async fn get_guilds(&self) -> Result<Vec<Guild>> {
        let url = format!("{}/users/@me/guilds", API_BASE);
        self.get(&url).await
    }

    /// ギルドのチャンネル一覧を取得
    pub async fn get_guild_channels(&self, guild_id: &str) -> Result<Vec<Channel>> {
        let url = format!("{}/guilds/{}/channels", API_BASE, guild_id);
        self.get(&url).await
    }

    /// DM チャンネル一覧を取得
    pub async fn get_dm_channels(&self) -> Result<Vec<Channel>> {
        let url = format!("{}/users/@me/channels", API_BASE);
        self.get(&url).await
    }

    /// チャンネルのメッセージを取得
    pub async fn get_messages(&self, channel_id: &str, limit: u8) -> Result<Vec<Message>> {
        let url = format!(
            "{}/channels/{}/messages?limit={}",
            API_BASE,
            channel_id,
            limit.min(100)
        );
        self.get(&url).await
    }

    /// メッセージを送信
    pub async fn send_message(&self, channel_id: &str, content: &str) -> Result<Message> {
        let url = format!("{}/channels/{}/messages", API_BASE, channel_id);
        let payload = CreateMessagePayload {
            content: content.to_string(),
        };
        self.post(&url, &payload).await
    }

    /// 現在のユーザー情報を取得
    #[allow(dead_code)]
    pub async fn get_current_user(&self) -> Result<User> {
        let url = format!("{}/users/@me", API_BASE);
        self.get(&url).await
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
