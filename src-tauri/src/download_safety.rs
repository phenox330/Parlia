//! Shared download-safety primitives used by both ModelManager (Whisper /
//! Parakeet ASR) and LlmModelManager. Centralizes URL allowlist, symlink
//! rejection, filename sanitization, and tar entry path validation so the
//! two managers can't drift on what counts as "safe to download and extract".
//!
//! Each manager passes its own host allowlist — keeping the two narrow
//! enforces least privilege (the LLM downloader can't reach Whisper hosts
//! and vice versa).

use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Component, Path};

/// Hard ceiling on any single download. Protects against a malformed
/// catalog entry with `size_mb = 0` or an absurdly small value that would
/// let a chunked response write gigabytes before any per-entry cap fires.
pub const ABSOLUTE_SIZE_CEILING_BYTES: u64 = 20 * 1024 * 1024 * 1024;

/// Derive a byte ceiling from a catalog's declared `size_mb`. Uses 2× the
/// claimed size + 100 MB headroom (some servers compress; final size can
/// drift from the catalog declaration), then clamps to the absolute ceiling.
pub fn size_cap_bytes(declared_mb: u64) -> u64 {
    let bytes = declared_mb
        .saturating_mul(2)
        .saturating_mul(1024 * 1024)
        .saturating_add(100 * 1024 * 1024);
    bytes.min(ABSOLUTE_SIZE_CEILING_BYTES)
}

/// Parse and allowlist a download URL. Enforces https + known host.
/// Call this for the initial URL AND for every redirect hop your client
/// follows (reqwest's `redirect::Policy::custom`).
pub fn validate_download_url(url: &reqwest::Url, allowlist: &[&str]) -> Result<()> {
    if url.scheme() != "https" {
        return Err(anyhow!(
            "Download URL must use https, got {:?}",
            url.scheme()
        ));
    }
    // Reject embedded credentials: a catalog entry like
    // `https://user:token@host/...` would forward the Authorization header
    // through every redirect and leak creds to the redirect target.
    if !url.username().is_empty() || url.password().is_some() {
        return Err(anyhow!("Download URL must not contain userinfo"));
    }
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("Download URL has no host"))?
        .to_ascii_lowercase();
    let ok = allowlist
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{}", h)));
    if !ok {
        return Err(anyhow!(
            "Download host {:?} is not in the allowlist",
            host
        ));
    }
    Ok(())
}

/// Reject a path target that has been replaced by a symlink. Opening or
/// renaming follows symlinks by default, which would let an attacker with
/// write access to the models directory redirect our writes to arbitrary
/// files (e.g. `~/.ssh/authorized_keys`). `symlink_metadata` inspects the
/// link itself without following — if the path doesn't exist at all, that's
/// fine (the create call will make a regular file).
pub fn reject_symlink(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(md) if md.file_type().is_symlink() => Err(anyhow!(
            "Refusing to write through a symlink at {}",
            path.display()
        )),
        _ => Ok(()),
    }
}

/// Reject filenames that could escape `models_dir` (defence in depth).
pub fn validate_filename(name: &str) -> Result<()> {
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || Path::new(name).is_absolute()
    {
        return Err(anyhow!("Unsafe model filename: {:?}", name));
    }
    Ok(())
}

/// Lowercase hex-encode a byte slice. Used to render a `sha2::Sha256`
/// digest for comparison against a pinned catalog hash.
pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Validate the path of a single tar archive entry — must be relative,
/// must not contain `..` segments, and must not be absolute. Prevents
/// path-traversal during extraction (tar-slip / zip-slip).
pub fn validate_tar_entry_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(anyhow!("Tar entry has empty path"));
    }
    if path.is_absolute() {
        return Err(anyhow!(
            "Tar entry has absolute path: {}",
            path.display()
        ));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => continue,
            Component::ParentDir => {
                return Err(anyhow!(
                    "Tar entry contains parent-dir segment: {}",
                    path.display()
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!(
                    "Tar entry has root or prefix segment: {}",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn url(s: &str) -> reqwest::Url {
        reqwest::Url::parse(s).expect("valid url")
    }

    #[test]
    fn allowlist_accepts_exact_and_subdomain() {
        let allow = &["example.com"];
        assert!(validate_download_url(&url("https://example.com/x"), allow).is_ok());
        assert!(validate_download_url(&url("https://cdn.example.com/x"), allow).is_ok());
    }

    #[test]
    fn allowlist_rejects_other_hosts() {
        let allow = &["example.com"];
        assert!(validate_download_url(&url("https://evil.com/x"), allow).is_err());
        // Substring match must NOT pass — bare `example.com.evil.com` is evil.
        assert!(validate_download_url(&url("https://example.com.evil.com/x"), allow).is_err());
    }

    #[test]
    fn allowlist_rejects_non_https() {
        let allow = &["example.com"];
        assert!(validate_download_url(&url("http://example.com/x"), allow).is_err());
    }

    #[test]
    fn allowlist_rejects_userinfo() {
        let allow = &["example.com"];
        assert!(validate_download_url(&url("https://user:pw@example.com/x"), allow).is_err());
    }

    #[test]
    fn validate_filename_blocks_traversal() {
        assert!(validate_filename("model.bin").is_ok());
        assert!(validate_filename("../model.bin").is_err());
        assert!(validate_filename("a/b").is_err());
        assert!(validate_filename("a\\b").is_err());
        assert!(validate_filename("/abs").is_err());
        assert!(validate_filename("").is_err());
    }

    #[test]
    fn validate_tar_entry_blocks_traversal() {
        assert!(validate_tar_entry_path(&PathBuf::from("nested/dir/file.bin")).is_ok());
        assert!(validate_tar_entry_path(&PathBuf::from("./file.bin")).is_ok());
        assert!(validate_tar_entry_path(&PathBuf::from("../escape.bin")).is_err());
        assert!(validate_tar_entry_path(&PathBuf::from("nested/../escape.bin")).is_err());
        assert!(validate_tar_entry_path(&PathBuf::from("/abs/path.bin")).is_err());
        assert!(validate_tar_entry_path(&PathBuf::from("")).is_err());
    }

    #[test]
    fn size_cap_handles_zero_and_overflow() {
        assert_eq!(size_cap_bytes(0), 100 * 1024 * 1024);
        assert_eq!(size_cap_bytes(500), 500 * 2 * 1024 * 1024 + 100 * 1024 * 1024);
        // Should never exceed the absolute ceiling, even on absurd inputs.
        assert_eq!(size_cap_bytes(u64::MAX), ABSOLUTE_SIZE_CEILING_BYTES);
    }
}
