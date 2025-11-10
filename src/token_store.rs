use anyhow::{Context, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "hakuhyo";
const USERNAME: &str = "discord_token";

/// トークンをシステムのキーチェーンに保存
///
/// # セキュリティ
/// - macOS: Keychain Services（システムレベルの暗号化）
/// - Windows: Credential Manager（DPAPI暗号化）
/// - Linux: Secret Service（GNOME Keyring/KWalletなど）
pub fn save_token(token: &str) -> Result<()> {
    log::debug!("Saving token to system keyring...");

    let entry = Entry::new(SERVICE_NAME, USERNAME)
        .context("Failed to create keyring entry")?;

    entry
        .set_password(token)
        .context("Failed to save token to keyring")?;

    log::info!("✓ Token saved to system keyring");
    Ok(())
}

/// トークンをシステムのキーチェーンから取得
pub fn load_token() -> Result<String> {
    log::debug!("Loading token from system keyring...");

    let entry = Entry::new(SERVICE_NAME, USERNAME)
        .context("Failed to create keyring entry")?;

    let token = entry
        .get_password()
        .context("No token found in keyring")?;

    log::info!("✓ Token loaded from system keyring");
    Ok(token)
}

/// トークンをシステムのキーチェーンから削除
///
/// # 用途
/// - 無効なトークンを削除する場合
/// - ユーザーが明示的にログアウトする場合
#[allow(dead_code)]
pub fn delete_token() -> Result<()> {
    log::debug!("Deleting token from system keyring...");

    let entry = Entry::new(SERVICE_NAME, USERNAME)
        .context("Failed to create keyring entry")?;

    entry
        .delete_credential()
        .context("Failed to delete token from keyring")?;

    log::info!("✓ Token deleted from system keyring");
    Ok(())
}
