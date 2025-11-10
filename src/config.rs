use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

/// アプリケーション設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// お気に入りチャンネルID一覧
    pub favorites: HashSet<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            favorites: HashSet::new(),
        }
    }
}

/// 設定ファイルのパスを取得
///
/// `~/.config/hakuhyo/favorites.json`
fn get_config_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .context("Failed to get config directory")?
        .join("hakuhyo");

    // ディレクトリが存在しない場合は作成
    if !config_dir.exists() {
        fs::create_dir_all(&config_dir)
            .context("Failed to create config directory")?;
        log::info!("Created config directory: {:?}", config_dir);
    }

    Ok(config_dir.join("favorites.json"))
}

/// 設定ファイルを読み込み
pub fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        log::info!("Config file not found, using default config");
        return Ok(Config::default());
    }

    let content = fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config file: {:?}", config_path))?;

    let config: Config = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {:?}", config_path))?;

    log::info!("✓ Loaded config from {:?}", config_path);
    log::debug!("Favorites count: {}", config.favorites.len());

    Ok(config)
}

/// 設定ファイルに保存
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;

    let content = serde_json::to_string_pretty(config)
        .context("Failed to serialize config")?;

    fs::write(&config_path, content)
        .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

    log::info!("✓ Saved config to {:?}", config_path);
    log::debug!("Favorites count: {}", config.favorites.len());

    Ok(())
}
