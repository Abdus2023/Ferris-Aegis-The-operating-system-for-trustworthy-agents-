//! Ferris Aegis Plugin System — Ed25519-signed plugin manifests.
//!
//! Every plugin must ship with a signed manifest. The manifest declares
//! the plugin's name, version, capabilities, and the SHA-256 hash of
//! the WASM binary. The manifest is signed with Ed25519, and the
//! signature is verified before the plugin is loaded.
//!
//! # Verification Flow
//!
//! 1. Load the manifest (JSON + signature bytes)
//! 2. Verify the Ed25519 signature against the manifest bytes
//! 3. Compute SHA-256 of the WASM binary
//! 4. Compare against the hash declared in the manifest
//! 5. Only if both checks pass, load the plugin

use anyhow::Context;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A plugin manifest declaring the plugin's identity and contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// The plugin's unique name.
    pub name: String,
    /// The plugin's semantic version.
    pub version: String,
    /// A human-readable description.
    pub description: String,
    /// The SHA-256 hash of the WASM binary (hex-encoded).
    pub wasm_hash: String,
    /// Capabilities required by this plugin.
    pub capabilities: Vec<String>,
    /// When this manifest was created.
    pub created_at: String,
    /// The Ed25519 public key of the signer (hex-encoded).
    pub signer_public_key: String,
}

/// A signed plugin manifest — the manifest plus its Ed25519 signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedManifest {
    /// The manifest.
    pub manifest: PluginManifest,
    /// The Ed25519 signature over the manifest JSON bytes (hex-encoded).
    pub signature: String,
}

/// The result of manifest verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyResult {
    /// Both the manifest signature and WASM hash are valid.
    Valid,
    /// Verification failed.
    Invalid(String),
}

impl VerifyResult {
    /// Whether the manifest is valid.
    pub fn is_valid(&self) -> bool {
        matches!(self, VerifyResult::Valid)
    }
}

/// A plugin keyring — stores trusted public keys for verification.
#[derive(Debug, Clone)]
pub struct PluginKeyring {
    /// Trusted verifying keys.
    trusted_keys: Vec<VerifyingKey>,
}

impl PluginKeyring {
    /// Create an empty keyring.
    pub fn new() -> Self {
        Self {
            trusted_keys: Vec::new(),
        }
    }

    /// Add a trusted public key from bytes.
    pub fn add_key(&mut self, key_bytes: &[u8; 32]) -> anyhow::Result<()> {
        let verifying_key = VerifyingKey::from_bytes(key_bytes)
            .map_err(|e| anyhow::anyhow!("invalid public key: {e}"))?;
        self.trusted_keys.push(verifying_key);
        Ok(())
    }

    /// Add a trusted public key from a hex-encoded string.
    pub fn add_key_from_hex(&mut self, hex_key: &str) -> anyhow::Result<()> {
        let bytes: Vec<u8> = hex::decode(hex_key)
            .context("failed to decode hex key")?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("key must be exactly 32 bytes"))?;
        self.add_key(&key_bytes)
    }

    /// Number of trusted keys.
    pub fn len(&self) -> usize {
        self.trusted_keys.len()
    }

    /// Whether the keyring is empty.
    pub fn is_empty(&self) -> bool {
        self.trusted_keys.is_empty()
    }

    /// Verify a signed manifest against the keyring.
    pub fn verify_manifest(&self, signed: &SignedManifest) -> VerifyResult {
        // 1. Find a trusted key that matches the signer
        let signer_key = match hex::decode(&signed.manifest.signer_public_key) {
            Ok(bytes) => bytes,
            Err(_) => return VerifyResult::Invalid("invalid signer public key hex".to_string()),
        };

        let verifying_key_bytes: [u8; 32] = match signer_key.try_into() {
            Ok(b) => b,
            Err(_) => return VerifyResult::Invalid("signer key must be 32 bytes".to_string()),
        };

        let verifying_key = match VerifyingKey::from_bytes(&verifying_key_bytes) {
            Ok(k) => k,
            Err(_) => return VerifyResult::Invalid("invalid signer key".to_string()),
        };

        if !self.trusted_keys.contains(&verifying_key) {
            return VerifyResult::Invalid("signer not in trusted keyring".to_string());
        }

        // 2. Verify the signature
        let manifest_bytes = match serde_json::to_vec(&signed.manifest) {
            Ok(b) => b,
            Err(_) => return VerifyResult::Invalid("failed to serialize manifest".to_string()),
        };

        let signature_bytes = match hex::decode(&signed.signature) {
            Ok(b) => b,
            Err(_) => return VerifyResult::Invalid("invalid signature hex".to_string()),
        };

        let signature = match Signature::from_slice(&signature_bytes) {
            Ok(s) => s,
            Err(_) => return VerifyResult::Invalid("invalid signature bytes".to_string()),
        };

        match verifying_key.verify(&manifest_bytes, &signature) {
            Ok(()) => VerifyResult::Valid,
            Err(e) => VerifyResult::Invalid(format!("signature verification failed: {e}")),
        }
    }

    /// Verify a signed manifest AND the WASM binary hash.
    pub fn verify_plugin(
        &self,
        signed: &SignedManifest,
        wasm_bytes: &[u8],
    ) -> VerifyResult {
        // First verify the manifest signature
        let manifest_result = self.verify_manifest(signed);
        if !manifest_result.is_valid() {
            return manifest_result;
        }

        // Then verify the WASM hash
        let computed_hash = compute_wasm_hash(wasm_bytes);
        if computed_hash != signed.manifest.wasm_hash {
            return VerifyResult::Invalid(format!(
                "WASM hash mismatch: expected {}, computed {}",
                signed.manifest.wasm_hash, computed_hash
            ));
        }

        VerifyResult::Valid
    }
}

impl Default for PluginKeyring {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute the SHA-256 hash of WASM bytes, returned as hex.
pub fn compute_wasm_hash(wasm_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(wasm_bytes);
    hex::encode(hasher.finalize())
}

/// Create a signing key and sign a manifest (for development/testing).
///
/// **Do not use in production** — keys should be generated offline
/// and stored securely. This is a convenience for testing.
pub fn sign_manifest(
    manifest: PluginManifest,
    signing_key: &SigningKey,
) -> SignedManifest {
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let signature = signing_key.sign(&manifest_bytes);

    SignedManifest {
        manifest,
        signature: hex::encode(signature.to_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;

    fn generate_signing_key() -> SigningKey {
        SigningKey::generate(&mut OsRng)
    }

    fn test_manifest(signer_pub_key: &str) -> PluginManifest {
        PluginManifest {
            name: "test-plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            wasm_hash: compute_wasm_hash(b"fake wasm bytes"),
            capabilities: vec!["file_read".to_string()],
            created_at: "2026-07-18T00:00:00Z".to_string(),
            signer_public_key: signer_pub_key.to_string(),
        }
    }

    #[test]
    fn sign_and_verify_manifest() {
        let signing_key = generate_signing_key();
        let verifying_key = signing_key.verifying_key();
        let pub_key_hex = hex::encode(verifying_key.to_bytes());

        let manifest = test_manifest(&pub_key_hex);
        let signed = sign_manifest(manifest, &signing_key);

        let mut keyring = PluginKeyring::new();
        keyring.add_key_from_hex(&pub_key_hex).unwrap();

        let result = keyring.verify_manifest(&signed);
        assert!(result.is_valid());
    }

    #[test]
    fn verify_manifest_with_wrong_key_fails() {
        let signing_key = generate_signing_key();
        let wrong_signing_key = generate_signing_key();
        let wrong_verifying_key = wrong_signing_key.verifying_key();
        let wrong_pub_hex = hex::encode(wrong_verifying_key.to_bytes());

        let real_pub_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let manifest = test_manifest(&real_pub_hex);
        let signed = sign_manifest(manifest, &signing_key);

        // Only trust the WRONG key
        let mut keyring = PluginKeyring::new();
        keyring.add_key_from_hex(&wrong_pub_hex).unwrap();

        let result = keyring.verify_manifest(&signed);
        assert!(!result.is_valid());
    }

    #[test]
    fn verify_plugin_with_matching_wasm_hash() {
        let signing_key = generate_signing_key();
        let pub_key_hex = hex::encode(signing_key.verifying_key().to_bytes());

        let wasm_bytes = b"fake wasm binary content";
        let manifest = PluginManifest {
            wasm_hash: compute_wasm_hash(wasm_bytes),
            ..test_manifest(&pub_key_hex)
        };

        let signed = sign_manifest(manifest, &signing_key);

        let mut keyring = PluginKeyring::new();
        keyring.add_key_from_hex(&pub_key_hex).unwrap();

        let result = keyring.verify_plugin(&signed, wasm_bytes);
        assert!(result.is_valid());
    }

    #[test]
    fn verify_plugin_with_mismatched_wasm_hash() {
        let signing_key = generate_signing_key();
        let pub_key_hex = hex::encode(signing_key.verifying_key().to_bytes());

        let wasm_bytes = b"correct wasm binary";
        let manifest = PluginManifest {
            wasm_hash: compute_wasm_hash(wasm_bytes),
            ..test_manifest(&pub_key_hex)
        };

        let signed = sign_manifest(manifest, &signing_key);

        let mut keyring = PluginKeyring::new();
        keyring.add_key_from_hex(&pub_key_hex).unwrap();

        // Verify with DIFFERENT wasm bytes
        let result = keyring.verify_plugin(&signed, b"tampered wasm binary");
        assert!(!result.is_valid());
        if let VerifyResult::Invalid(reason) = result {
            assert!(reason.contains("hash mismatch"));
        }
    }

    #[test]
    fn wasm_hash_deterministic() {
        let bytes = b"some wasm content";
        let hash1 = compute_wasm_hash(bytes);
        let hash2 = compute_wasm_hash(bytes);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn wasm_hash_differs_for_different_content() {
        let hash1 = compute_wasm_hash(b"wasm v1");
        let hash2 = compute_wasm_hash(b"wasm v2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn empty_keyring_rejects_everything() {
        let signing_key = generate_signing_key();
        let pub_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let manifest = test_manifest(&pub_key_hex);
        let signed = sign_manifest(manifest, &signing_key);

        let keyring = PluginKeyring::new();
        let result = keyring.verify_manifest(&signed);
        assert!(!result.is_valid());
    }
}
