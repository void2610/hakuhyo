use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// トークンファイルのパスを取得
///
/// `~/.config/hakuhyo/token.txt`
fn get_token_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Failed to get config directory")?
        .join("hakuhyo");

    // ディレクトリが存在しない場合は作成
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .context("Failed to create config directory")?;
        log::debug!("Created config directory: {:?}", config_dir);
    }

    Ok(config_dir.join("token.txt"))
}

/// トークンをファイルに保存
///
/// # セキュリティ
/// - ファイルパーミッション: 0600（所有者のみ読み書き可能）
/// - 保存先: ~/.config/hakuhyo/token.txt
/// - ⚠️ 平文で保存されるため、バックアップやクラウド同期に注意
pub fn save_token(token: &str) -> Result<()> {
    log::debug!("Saving token to file...");

    let token_path = get_token_path()?;

    // トークンをファイルに書き込み
    fs::write(&token_path, token)
        .with_context(|| format!("Failed to write token file: {:?}", token_path))?;

    // Unix系OSの場合、ファイルパーミッションを 0600 に設定（所有者のみ読み書き可能）
    #[cfg(unix)]
    {
        let metadata = fs::metadata(&token_path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&token_path, permissions)?;
        log::debug!("Set token file permissions to 0600");
    }

    log::info!("✓ Token saved to {:?}", token_path);
    Ok(())
}

/// トークンをファイルから読み込み
pub fn load_token() -> Result<String> {
    log::debug!("Loading token from file...");

    let token_path = get_token_path()?;

    if !token_path.exists() {
        anyhow::bail!("Token file not found");
    }

    let token = fs::read_to_string(&token_path)
        .with_context(|| format!("Failed to read token file: {:?}", token_path))?;

    log::info!("✓ Token loaded from {:?}", token_path);
    Ok(token.trim().to_string())
}

/// トークンをファイルから削除
///
/// # 用途
/// - 無効なトークンを削除する場合
/// - ユーザーが明示的にログアウトする場合
#[allow(dead_code)]
pub fn delete_token() -> Result<()> {
    log::debug!("Deleting token file...");

    let token_path = get_token_path()?;

    if token_path.exists() {
        fs::remove_file(&token_path)
            .with_context(|| format!("Failed to delete token file: {:?}", token_path))?;
        log::info!("✓ Token file deleted: {:?}", token_path);
    } else {
        log::debug!("Token file does not exist, nothing to delete");
    }

    Ok(())
}
