use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use futures::{SinkExt, StreamExt};
use qr2term::print_qr;
use rsa::{pkcs8::EncodePublicKey, Oaep, RsaPrivateKey, RsaPublicKey};
use serde::Deserialize;
use serde_json::json;
use sha2::Sha256;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::token_store;

const REMOTE_AUTH_URL: &str = "wss://remote-auth-gateway.discord.gg/?v=2";
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

/// Remote Auth WebSocketメッセージ
#[derive(Debug, Deserialize)]
struct RemoteAuthMessage {
    op: String,
    #[serde(flatten)]
    data: serde_json::Value,
}

/// QRコード認証を実行してDiscordトークンを取得
///
/// # フロー
/// 1. Remote Auth WebSocketサーバーに接続（v=2）
/// 2. RSA鍵ペアを生成
/// 3. 公開鍵を送信
/// 4. QRコードを生成してターミナルに表示
/// 5. ユーザーがモバイルアプリでスキャン・承認
/// 6. トークンを取得
pub async fn authenticate_with_qr() -> Result<String> {
    log::info!("Starting QR code authentication...");

    // WebSocket接続（必要なヘッダーを追加）
    log::debug!("Connecting to Remote Auth WebSocket: {}", REMOTE_AUTH_URL);

    use tokio_tungstenite::tungstenite::client::IntoClientRequest;

    // URLからWebSocketリクエストを作成し、カスタムヘッダーを追加
    let mut request = REMOTE_AUTH_URL
        .into_client_request()
        .context("Failed to create WebSocket request")?;

    request.headers_mut().insert(
        "Origin",
        "https://discord.com".parse()
            .context("Failed to parse Origin header")?
    );
    request.headers_mut().insert(
        "User-Agent",
        USER_AGENT.parse()
            .context("Failed to parse User-Agent header")?
    );

    let (ws_stream, _) = connect_async(request)
        .await
        .context("Failed to connect to Discord Remote Auth")?;
    log::info!("Connected to Remote Auth server");

    let (mut write, mut read) = ws_stream.split();

    // Hello メッセージを待機
    let hello_msg = read
        .next()
        .await
        .context("No hello message received")?
        .context("WebSocket error")?;

    let hello: RemoteAuthMessage =
        serde_json::from_str(&hello_msg.to_string()).context("Failed to parse hello")?;

    if hello.op != "hello" {
        anyhow::bail!("Expected hello, got: {}", hello.op);
    }

    let heartbeat_interval = hello.data["heartbeat_interval"]
        .as_u64()
        .context("No heartbeat_interval in hello")?;
    log::debug!("Heartbeat interval: {}ms", heartbeat_interval);

    // ハートビート間隔を設定
    let mut heartbeat_timer = tokio::time::interval(tokio::time::Duration::from_millis(heartbeat_interval));
    heartbeat_timer.tick().await; // 最初のtickをスキップ

    // RSA鍵ペアを生成（2048ビット）
    log::debug!("Generating RSA key pair...");
    let mut rng = rand::thread_rng();
    let private_key = RsaPrivateKey::new(&mut rng, 2048)
        .context("Failed to generate RSA private key")?;
    let public_key = RsaPublicKey::from(&private_key);

    // 公開鍵をSPKI (SubjectPublicKeyInfo) 形式でエンコード
    let public_key_der = public_key
        .to_public_key_der()
        .context("Failed to encode public key")?;
    let public_key_b64 = general_purpose::STANDARD.encode(public_key_der.as_bytes());

    // init メッセージを送信
    let init_msg = json!({
        "op": "init",
        "encoded_public_key": public_key_b64
    });
    write
        .send(Message::Text(init_msg.to_string()))
        .await
        .context("Failed to send init")?;
    log::debug!("Sent init with public key");

    // メッセージ受信とハートビート送信を並行処理
    let mut token = String::new();
    loop {
        tokio::select! {
            // ハートビート送信
            _ = heartbeat_timer.tick() => {
                let heartbeat = json!({"op": "heartbeat"}).to_string();
                if let Err(e) = write.send(Message::Text(heartbeat)).await {
                    log::error!("Failed to send heartbeat: {}", e);
                    break;
                }
                log::debug!("Sent heartbeat");
            }
            // メッセージ受信
            msg_result = read.next() => {
                let msg = match msg_result {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => {
                        anyhow::bail!("WebSocket error: {}", e);
                    }
                    None => {
                        anyhow::bail!("WebSocket connection closed");
                    }
                };

                let data: RemoteAuthMessage = serde_json::from_str(&msg.to_string())?;
                log::debug!("Received op: {}", data.op);

                match data.op.as_str() {
                    "nonce_proof" => {
                        // Nonceを復号化
                        let encrypted_nonce = data.data["encrypted_nonce"]
                            .as_str()
                            .context("No encrypted_nonce")?;

                        let encrypted_bytes = general_purpose::STANDARD
                            .decode(encrypted_nonce)
                            .context("Failed to decode nonce")?;

                        let padding = Oaep::new::<Sha256>();
                        let decrypted_nonce = private_key
                            .decrypt(padding, &encrypted_bytes)
                            .context("Failed to decrypt nonce")?;

                        // SHA256ハッシュを計算
                        use sha2::Digest;
                        let mut hasher = Sha256::new();
                        hasher.update(&decrypted_nonce);
                        let nonce_hash = hasher.finalize();
                        let proof = general_purpose::URL_SAFE_NO_PAD.encode(nonce_hash);

                        // nonce_proof を送信
                        let proof_msg = json!({
                            "op": "nonce_proof",
                            "proof": proof
                        });
                        write.send(Message::Text(proof_msg.to_string())).await?;
                        log::debug!("Sent nonce_proof");
                    }
                    "pending_remote_init" => {
                        // フィンガープリント取得
                        let fingerprint = data.data["fingerprint"]
                            .as_str()
                            .context("No fingerprint")?;

                        log::info!("Fingerprint: {}", fingerprint);

                        // QRコード URL を生成
                        let qr_url = format!("https://discord.com/ra/{}", fingerprint);

                        // QRコードを生成・表示
                        println!("\n╔══════════════════════════════════════╗");
                        println!("║      Discord QRコードログイン        ║");
                        println!("╚══════════════════════════════════════╝");
                        println!("\nモバイルのDiscordアプリで以下のQRコードをスキャンしてください：\n");

                        // QRコードを表示（エラーが発生した場合はURLを表示）
                        if let Err(e) = print_qr(&qr_url) {
                            log::warn!("Failed to display QR code: {}. Showing URL instead.", e);
                            println!("QRコード表示エラー。以下のURLをブラウザで開いてください：");
                            println!("{}", qr_url);
                        }

                        println!("\n認証を待っています...");
                        println!("（モバイルアプリで「ログイン」→「QRコードでログイン」をタップ）");
                    }
                    "pending_ticket" => {
                        log::info!("User scanned QR code");
                        println!("\n✓ QRコードがスキャンされました");
                        println!("  モバイルアプリで「はい、ログインします」をタップしてください");
                    }
                    "pending_login" => {
                        // ユーザーが承認、トークンを取得
                        let ticket = data.data["ticket"]
                            .as_str()
                            .context("No ticket in pending_login")?;

                        log::debug!("Got ticket, exchanging for token...");

                        // トークン取得API呼び出し
                        let client = reqwest::Client::new();
                        let token_response = client
                            .post("https://discord.com/api/v9/users/@me/remote-auth/login")
                            .json(&json!({"ticket": ticket}))
                            .send()
                            .await
                            .context("Failed to exchange ticket for token")?;

                        #[derive(Deserialize)]
                        struct TokenResponse {
                            encrypted_token: String,
                        }

                        let token_data: TokenResponse = token_response
                            .json()
                            .await
                            .context("Failed to parse token response")?;

                        // トークンを復号化
                        let encrypted_token_bytes = general_purpose::STANDARD
                            .decode(&token_data.encrypted_token)
                            .context("Failed to decode encrypted token")?;

                        let padding = Oaep::new::<Sha256>();
                        let decrypted_token = private_key
                            .decrypt(padding, &encrypted_token_bytes)
                            .context("Failed to decrypt token")?;

                        token = String::from_utf8(decrypted_token)
                            .context("Invalid UTF-8 in decrypted token")?;

                        log::info!("Authentication successful");
                        println!("✓ 認証に成功しました！\n");
                        break;
                    }
                    "cancel" => {
                        anyhow::bail!("Authentication was cancelled");
                    }
                    "heartbeat_ack" => {
                        // ハートビートACKは無視
                    }
                    _ => {
                        log::debug!("Ignoring unknown op: {}", data.op);
                    }
                }
            }
        }

        if !token.is_empty() {
            break;
        }
    }

    if token.is_empty() {
        anyhow::bail!("Failed to get token");
    }

    Ok(token)
}

/// 保存されたトークンを検証
///
/// Discord APIの `/users/@me` エンドポイントを使用してトークンの有効性を確認
async fn validate_stored_token(token: &str) -> bool {
    log::debug!("Validating stored token...");

    let client = reqwest::Client::new();
    let response = client
        .get("https://discord.com/api/v10/users/@me")
        .header("Authorization", token)
        .header("User-Agent", USER_AGENT)
        .send()
        .await;

    match response {
        Ok(resp) if resp.status().is_success() => {
            log::info!("✓ Stored token is valid");
            true
        }
        Ok(resp) => {
            log::warn!("✗ Stored token is invalid: {}", resp.status());
            false
        }
        Err(e) => {
            log::error!("Failed to validate token: {}", e);
            false
        }
    }
}

/// トークンを取得（キーチェーン → QRコード認証）
///
/// # 認証フロー
/// 1. システムキーチェーンから読み込み → 検証
/// 2. QRコード認証を実行 → キーチェーンに保存
///
/// # エラー
/// - 全ての認証方法が失敗した場合
pub async fn get_or_authenticate_token() -> Result<String> {
    // 1. キーチェーンから取得を試行
    if let Ok(token) = tokio::task::spawn_blocking(|| token_store::load_token()).await? {
        log::info!("Token found in keyring, validating...");
        if validate_stored_token(&token).await {
            return Ok(token);
        } else {
            log::warn!("Stored token is invalid, will re-authenticate");
            // 無効なトークンは削除
            let _ = tokio::task::spawn_blocking(|| token_store::delete_token()).await;
        }
    } else {
        log::debug!("No token found in keyring");
    }

    // 2. QRコード認証を実行
    log::info!("Starting QR code authentication...");
    let token = authenticate_with_qr().await?;

    // 3. 取得したトークンをキーチェーンに保存
    let token_clone = token.clone();
    tokio::task::spawn_blocking(move || {
        if let Err(e) = token_store::save_token(&token_clone) {
            log::error!("Failed to save token to keyring: {}", e);
        }
    })
    .await?;

    Ok(token)
}
