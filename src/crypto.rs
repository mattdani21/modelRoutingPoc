//! Application-level encryption for the evidence ledger.
//!
//! The store keeps one JSON payload per run. When a database key is configured
//! the payload is sealed with AES-256-GCM before it reaches SQLite, so a stolen
//! database file does not disclose model output, business context, or reviewer
//! identity. Records written without a key stay in plain text and carry no
//! `enc:v1:` marker, so a development database keeps working after a key is added
//! for a later run.

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};

const MARKER: &str = "enc:v1:";
const NONCE_LEN: usize = 12;

/// A payload cipher. `Disabled` is a transparent pass-through used for the
/// loopback development mode.
pub enum Cipher {
    Disabled,
    Enabled(Box<Aes256Gcm>),
}

impl Cipher {
    /// Build a cipher from a base64 environment variable. An empty or missing
    /// value disables encryption. A present value must decode to exactly 32
    /// bytes.
    pub fn from_env(variable: &str) -> Result<Self> {
        match std::env::var(variable) {
            Ok(raw) if !raw.trim().is_empty() => Self::from_base64_key(raw.trim()),
            _ => Ok(Cipher::Disabled),
        }
    }

    pub fn from_base64_key(encoded: &str) -> Result<Self> {
        let bytes = STANDARD
            .decode(encoded)
            .context("The database key must be base64")?;
        if bytes.len() != 32 {
            bail!("The database key must decode to 32 bytes for AES-256-GCM");
        }
        let key = Key::<Aes256Gcm>::from_slice(&bytes);
        Ok(Cipher::Enabled(Box::new(Aes256Gcm::new(key))))
    }

    pub fn enabled(&self) -> bool {
        matches!(self, Cipher::Enabled(_))
    }

    /// Seal a payload. When disabled the plain text is returned unchanged.
    pub fn seal(&self, plaintext: &str) -> Result<String> {
        match self {
            Cipher::Disabled => Ok(plaintext.to_string()),
            Cipher::Enabled(cipher) => {
                let mut nonce_bytes = [0u8; NONCE_LEN];
                getrandom::getrandom(&mut nonce_bytes)
                    .map_err(|error| anyhow::anyhow!("The nonce generator failed: {error}"))?;
                let nonce = Nonce::from_slice(&nonce_bytes);
                let ciphertext = cipher
                    .encrypt(nonce, plaintext.as_bytes())
                    .map_err(|_| anyhow::anyhow!("The payload could not be encrypted"))?;
                let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
                blob.extend_from_slice(&nonce_bytes);
                blob.extend_from_slice(&ciphertext);
                Ok(format!("{MARKER}{}", STANDARD.encode(blob)))
            }
        }
    }

    /// Open a stored payload. A record without the marker is treated as legacy
    /// plain text. A marked record requires a configured key.
    pub fn open(&self, stored: &str) -> Result<String> {
        let Some(encoded) = stored.strip_prefix(MARKER) else {
            return Ok(stored.to_string());
        };
        let cipher = match self {
            Cipher::Enabled(cipher) => cipher,
            Cipher::Disabled => {
                bail!("An encrypted record was found but no database key is configured")
            }
        };
        let blob = STANDARD
            .decode(encoded)
            .context("The stored ciphertext is not valid base64")?;
        if blob.len() <= NONCE_LEN {
            bail!("The stored ciphertext is too short");
        }
        let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| anyhow::anyhow!("The payload could not be decrypted with this key"))?;
        String::from_utf8(plaintext).context("The decrypted payload is not valid UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cipher() -> Cipher {
        // 32 zero bytes, base64 encoded. Test only.
        Cipher::from_base64_key(&STANDARD.encode([0u8; 32])).unwrap()
    }

    #[test]
    fn disabled_cipher_is_transparent() {
        let cipher = Cipher::Disabled;
        assert_eq!(cipher.seal("hello").unwrap(), "hello");
        assert_eq!(cipher.open("hello").unwrap(), "hello");
    }

    #[test]
    fn enabled_cipher_round_trips() {
        let cipher = test_cipher();
        let sealed = cipher.seal("confidential payload").unwrap();
        assert!(sealed.starts_with(MARKER));
        assert!(!sealed.contains("confidential"));
        assert_eq!(cipher.open(&sealed).unwrap(), "confidential payload");
    }

    #[test]
    fn enabled_cipher_reads_legacy_plaintext() {
        let cipher = test_cipher();
        assert_eq!(cipher.open("{\"run_id\":\"x\"}").unwrap(), "{\"run_id\":\"x\"}");
    }

    #[test]
    fn wrong_key_cannot_open_sealed_payload() {
        let sealed = test_cipher().seal("secret").unwrap();
        let other = Cipher::from_base64_key(&STANDARD.encode([9u8; 32])).unwrap();
        assert!(other.open(&sealed).is_err());
    }

    #[test]
    fn key_must_be_32_bytes() {
        assert!(Cipher::from_base64_key(&STANDARD.encode([0u8; 16])).is_err());
    }
}
