//! OS keychain wrapper for the creem API key (full-access secret).
//! The key is never written to disk in plaintext and never returned to JS.

use keyring::Entry;

const SERVICE: &str = "com.kapishdima.creembar";
const ACCOUNT: &str = "creem_api_key";

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|e| e.to_string())
}

pub fn get_api_key() -> Option<String> {
    match entry().ok()?.get_password() {
        Ok(k) if !k.is_empty() => Some(k),
        _ => None,
    }
}

pub fn set_api_key(key: &str) -> Result<(), String> {
    entry()?.set_password(key).map_err(|e| e.to_string())
}

pub fn clear_api_key() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
