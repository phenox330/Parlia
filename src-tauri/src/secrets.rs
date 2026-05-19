// OS keychain wrapper for secrets that must never hit disk in plaintext.
//
// Wraps the `keyring` crate (macOS Keychain, Windows Credential Manager,
// Linux Secret Service) with a tiny domain-specific API: the rest of the
// codebase asks for a named secret (`SecretName::Anthropic`,
// `SecretName::OpenAiCompat`) and never touches the underlying keyring
// service / account strings directly.
//
// Reads return `Option<String>` so callers can treat "not configured" as a
// regular state without parsing errors. Writes return `Result` so we can
// surface platform failures (e.g. user-denied Keychain access) up to the
// IPC layer.

use keyring::Entry;

const KEYCHAIN_SERVICE: &str = "com.parlia.app";

#[derive(Copy, Clone)]
pub enum SecretName {
    Anthropic,
    OpenAiCompat,
}

impl SecretName {
    fn account(self) -> &'static str {
        match self {
            SecretName::Anthropic => "anthropic_api_key",
            SecretName::OpenAiCompat => "openai_compat_api_key",
        }
    }
}

fn entry(name: SecretName) -> keyring::Result<Entry> {
    Entry::new(KEYCHAIN_SERVICE, name.account())
}

/// Store a secret. Empty values delete the entry (treating "" the same as
/// "no key configured" matches how the UI write path already behaves).
pub fn set_secret(name: SecretName, value: &str) -> keyring::Result<()> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return delete_secret(name);
    }
    let entry = entry(name)?;
    entry.set_password(trimmed)
}

/// Fetch a secret. `Ok(None)` when the entry is missing (the common "user
/// hasn't configured a key" case); `Err` on actual platform failure.
pub fn get_secret(name: SecretName) -> keyring::Result<Option<String>> {
    let entry = entry(name)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(value)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Remove a secret. `NoEntry` is treated as success — deleting a thing that
/// already doesn't exist is what the caller wants.
pub fn delete_secret(name: SecretName) -> keyring::Result<()> {
    let entry = entry(name)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e),
    }
}

/// Cheap "is anything stored under this name?" check for the UI.
pub fn has_secret(name: SecretName) -> bool {
    matches!(get_secret(name), Ok(Some(v)) if !v.is_empty())
}
