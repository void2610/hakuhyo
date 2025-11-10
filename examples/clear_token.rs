use anyhow::Result;
use keyring::Entry;

const SERVICE_NAME: &str = "hakuhyo";
const USERNAME: &str = "discord_token";

fn main() -> Result<()> {
    println!("Clearing saved Discord token from keychain...");

    let entry = Entry::new(SERVICE_NAME, USERNAME)?;

    match entry.delete_credential() {
        Ok(_) => {
            println!("✓ Token cleared successfully");
            Ok(())
        }
        Err(e) => {
            println!("Note: {}", e);
            println!("(This is normal if no token was saved)");
            Ok(())
        }
    }
}
