use std::path::PathBuf;

use anyhow::{Context, Result};

pub fn account_path(account: &str) -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let path = PathBuf::from(home)
        .join("Library/Containers/com.ranchero.NetNewsWire-Evergreen/Data/Library/Application Support/NetNewsWire/Accounts")
        .join(account);
    if !path.exists() {
        anyhow::bail!("Account directory not found: {}", path.display());
    }
    Ok(path)
}

pub fn db_path(account: &str) -> Result<PathBuf> {
    Ok(account_path(account)?.join("DB.sqlite3"))
}

pub fn feed_settings_path(account: &str) -> Result<PathBuf> {
    Ok(account_path(account)?.join("FeedSettings.db"))
}

pub fn opml_path(account: &str) -> Result<PathBuf> {
    Ok(account_path(account)?.join("Subscriptions.opml"))
}
