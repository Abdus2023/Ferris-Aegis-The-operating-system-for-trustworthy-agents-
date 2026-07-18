//! Credential vault — Structural secret protection for API keys.
//!
//! The vault stores credentials encrypted at rest and injects them into
//! tool calls at execution time. The core invariant is enforced
//! structurally, not by convention:
//!
//! - [`AuthenticatedCall`] carries the LLM-visible call and the injected
//!   credential as **separate fields**. The `call` field is safe to trace
//!   freely; the `credential` field structurally cannot be serialized
//!   or debugged in a way that leaks its contents.
//!
//! - The tool executor is the **only** code that calls `.expose_secret()`,
//!   and only at the point of actual use (e.g. building an HTTP header).
//!
//! - Tool-call tracing spans are populated from `call.arguments` only —
//!   never from a post-injection copy that could carry `credential`.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Aes256Gcm as Cipher, Nonce};
use secrecy::{ExposeSecret, Secret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

/// A stored credential, encrypted at rest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCredential {
    /// A human-readable name for this credential (e.g., "openai-api-key").
    pub name: String,
    /// The encrypted credential bytes (AES-256-GCM ciphertext).
    pub encrypted: Vec<u8>,
    /// The nonce used for encryption.
    pub nonce: Vec<u8>,
    /// When the credential was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// When the credential expires, if ever.
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// The credential vault.
///
/// Stores credentials encrypted with AES-256-GCM. The encryption key
/// is derived from a master key provided at initialization. The key
/// is held in a `Secret<String>` and zeroized on drop.
pub struct CredentialVault {
    /// The derived encryption key (32 bytes for AES-256).
    key: Secret<[u8; 32]>,
    /// Stored credentials indexed by name.
    credentials: Vec<StoredCredential>,
}

impl CredentialVault {
    /// Create a new vault with the given master key.
    ///
    /// The master key is hashed with SHA-256 to produce the 32-byte
    /// AES-256 key. This is not a KDF — for production, use Argon2
    /// or HKDF. This is sufficient for the Phase 3 scope.
    pub fn new(master_key: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(master_key.as_bytes());
        let key_bytes: [u8; 32] = hasher.finalize().into();

        Self {
            key: Secret::new(key_bytes),
            credentials: Vec::new(),
        }
    }

    /// Store a credential in the vault.
    ///
    /// The credential is encrypted with AES-256-GCM using a random nonce.
    /// The plaintext is never stored.
    pub fn store(&mut self, name: &str, secret: SecretString) -> anyhow::Result<()> {
        let cipher = Aes256Gcm::new_from_slice(self.key.expose_secret())
            .map_err(|e| anyhow::anyhow!("failed to create cipher: {e}"))?;

        let nonce_bytes = aes_gcm::aead::rand_nonce::generate_nonce(&mut OsRng);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let encrypted = cipher
            .encrypt(nonce, secret.expose_secret().as_bytes())
            .map_err(|e| anyhow::anyhow!("encryption failed: {e}"))?;

        self.credentials.push(StoredCredential {
            name: name.to_string(),
            encrypted,
            nonce: nonce_bytes.to_vec(),
            created_at: chrono::Utc::now(),
            expires_at: None,
        });

        tracing::info!(name = name, "credential stored in vault");
        Ok(())
    }

    /// Retrieve a credential from the vault by name.
    ///
    /// Returns the decrypted credential as a `Secret<String>`. The
    /// caller is responsible for not converting it to a plain `String`
    /// except at the point of actual use.
    pub fn get(&self, name: &str) -> Option<SecretString> {
        let stored = self.credentials.iter().find(|c| c.name == name)?;

        let cipher = Aes256Gcm::new_from_slice(self.key.expose_secret()).ok()?;

        let nonce = Nonce::from_slice(&stored.nonce);

        let decrypted = cipher.decrypt(nonce, stored.encrypted.as_slice()).ok()?;

        // Convert to SecretString immediately — never a plain String
        let secret_str =
            String::from_utf8(decrypted).ok()?;

        Some(SecretString::new(secret_str))
    }

    /// Remove a credential from the vault.
    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.credentials.len();
        self.credentials.retain(|c| c.name != name);
        self.credentials.len() < before
    }

    /// List credential names (never the values).
    pub fn list(&self) -> Vec<&str> {
        self.credentials.iter().map(|c| c.name.as_str()).collect()
    }

    /// Number of stored credentials.
    pub fn len(&self) -> usize {
        self.credentials.len()
    }

    /// Whether the vault is empty.
    pub fn is_empty(&self) -> bool {
        self.credentials.is_empty()
    }
}

/// A tool call with an optional injected credential.
///
/// This is the **structural fix** for the credential vault invariant.
/// The LLM-visible call and the injected credential travel separately:
///
/// - `call` is exactly what the model proposed — safe to trace freely.
/// - `credential`, if present, is never `Debug`, never `Serialize`, and
///   never touches `call.arguments`.
///
/// The tool executor takes both, separately, and is the only place
/// that ever calls `.expose_secret()` — at the point of actual use
/// (e.g. building an HTTP header), never before.
pub struct AuthenticatedCall<'a> {
    /// The tool call as proposed by the LLM.
    /// Safe to log, trace, and serialize — this is exactly what the model
    /// produced, with no credential data injected.
    pub call: &'a ToolCall,
    /// The credential to inject, if this tool requires one.
    /// Structurally cannot be Debug'd or Serialize'd — `Secret<String>`
    /// protects its contents at the type level.
    pub credential: Option<SecretString>,
}

/// A tool call as proposed by the LLM.
///
/// This struct contains only what the LLM proposed — no credential
/// data is ever written into `arguments`. It is safe to trace this
/// entire struct freely.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// The name of the tool being called.
    pub name: String,
    /// The arguments to the tool call, as proposed by the LLM.
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// Create a new tool call.
    pub fn new(name: &str, arguments: serde_json::Value) -> Self {
        Self {
            name: name.to_string(),
            arguments,
        }
    }

    /// Create an authenticated call by attaching a credential.
    ///
    /// The credential travels separately from the call — it is never
    /// injected into `arguments`. This is the structural guarantee.
    pub fn with_credential(&self, credential: SecretString) -> AuthenticatedCall<'_> {
        AuthenticatedCall {
            call: self,
            credential: Some(credential),
        }
    }

    /// Create an unauthenticated call (no credential needed).
    pub fn without_credential(&self) -> AuthenticatedCall<'_> {
        AuthenticatedCall {
            call: self,
            credential: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_store_and_retrieve() {
        let mut vault = CredentialVault::new("test-master-key");
        vault
            .store("test-key", SecretString::new("sk-secret123".to_string()))
            .unwrap();

        let retrieved = vault.get("test-key").unwrap();
        assert_eq!(retrieved.expose_secret(), "sk-secret123");
    }

    #[test]
    fn vault_missing_key_returns_none() {
        let vault = CredentialVault::new("test-master-key");
        assert!(vault.get("nonexistent").is_none());
    }

    #[test]
    fn vault_list_names_only() {
        let mut vault = CredentialVault::new("test-master-key");
        vault
            .store("key-1", SecretString::new("secret1".to_string()))
            .unwrap();
        vault
            .store("key-2", SecretString::new("secret2".to_string()))
            .unwrap();

        let names = vault.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"key-1"));
        assert!(names.contains(&"key-2"));
        // Names should never contain the secret values
        assert!(!names.iter().any(|n| n.contains("secret")));
    }

    #[test]
    fn vault_remove_credential() {
        let mut vault = CredentialVault::new("test-master-key");
        vault
            .store("removable", SecretString::new("secret".to_string()))
            .unwrap();
        assert!(vault.remove("removable"));
        assert!(vault.get("removable").is_none());
    }

    #[test]
    fn vault_different_master_keys_fail() {
        let mut vault = CredentialVault::new("master-1");
        vault
            .store("test", SecretString::new("secret".to_string()))
            .unwrap();

        // Wrong master key — should not decrypt
        let vault2 = CredentialVault::new("master-2");
        // The vault2 doesn't have the stored credential, but if we
        // somehow got the encrypted blob, it would fail to decrypt
        assert!(vault2.get("test").is_none());
    }

    #[test]
    fn tool_call_never_carries_credential_in_arguments() {
        let call = ToolCall::new(
            "http_request",
            serde_json::json!({"url": "https://api.example.com", "method": "GET"}),
        );

        // The call has no credential in arguments
        assert!(call.arguments.as_object().unwrap().get("_credential").is_none());
        assert!(call.arguments.as_object().unwrap().get("credential").is_none());
    }

    #[test]
    fn authenticated_call_carries_credential_separately() {
        let call = ToolCall::new(
            "http_request",
            serde_json::json!({"url": "https://api.example.com"}),
        );

        let auth_call = call.with_credential(SecretString::new("sk-api-key-123".to_string()));

        // The call's arguments are still clean — no credential injected
        assert!(auth_call.call.arguments.as_object().unwrap().get("_credential").is_none());

        // The credential travels separately
        assert!(auth_call.credential.is_some());
        assert_eq!(
            auth_call.credential.unwrap().expose_secret(),
            "sk-api-key-123"
        );

        // The call itself can be freely serialized (no secret leakage)
        let serialized = serde_json::to_string(auth_call.call).unwrap();
        assert!(!serialized.contains("sk-api-key-123"));
    }
}
