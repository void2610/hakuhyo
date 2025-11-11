use anyhow::Result;
use std::fs;
use std::path::PathBuf;

fn get_token_path() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to get config directory"))?
        .join("hakuhyo");

    Ok(config_dir.join("token.txt"))
}

fn main() -> Result<()> {
    println!("Clearing saved Discord token from file...");

    let token_path = get_token_path()?;

    if token_path.exists() {
        fs::remove_file(&token_path)?;
        println!("✓ Token cleared successfully: {:?}", token_path);
    } else {
        println!("Note: Token file not found");
        println!("(This is normal if no token was saved)");
    }

    Ok(())
}
