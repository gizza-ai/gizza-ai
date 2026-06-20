//! gizza-ai/protect-pdf core — add password encryption to a PDF using the PDF
//! standard security handler. Pure-Rust (`lopdf`). No wafer/wasm-bindgen deps.
//!
//! Uses AES-256 (encryption "V5" / revision 6, the strongest standard handler):
//! a random 256-bit file key is generated and wrapped with the supplied
//! password(s). Opening the result in any PDF viewer then requires the password.

use std::collections::BTreeMap;
use std::sync::Arc;

use lopdf::encryption::crypt_filters::{Aes256CryptFilter, CryptFilter};
use lopdf::{Document, EncryptionState, EncryptionVersion, Permissions};
use rand::RngCore;

/// Encrypt `pdf` with `user_password` (required to open the document) and
/// `owner_password` (controls permissions; falls back to the user password when
/// empty). Returns the encrypted PDF bytes.
pub fn run(pdf: &[u8], user_password: &str, owner_password: &str) -> Result<Vec<u8>, String> {
    if user_password.is_empty() {
        return Err("a password is required to protect the PDF".into());
    }
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    if doc.is_encrypted() {
        return Err("this PDF is already password-protected".into());
    }

    let owner = if owner_password.is_empty() {
        user_password
    } else {
        owner_password
    };

    // Random 256-bit file encryption key for AES-256 (V5/R6).
    let mut file_encryption_key = [0u8; 32];
    rand::rng().fill_bytes(&mut file_encryption_key);

    let crypt_filter: Arc<dyn CryptFilter> = Arc::new(Aes256CryptFilter);
    let version = EncryptionVersion::V5 {
        encrypt_metadata: true,
        crypt_filters: BTreeMap::from([(b"StdCF".to_vec(), crypt_filter)]),
        file_encryption_key: &file_encryption_key,
        stream_filter: b"StdCF".to_vec(),
        string_filter: b"StdCF".to_vec(),
        owner_password: owner,
        user_password,
        // Grant all standard permissions — the goal is an open password, not a
        // restricted document. Holders of the password get full use.
        permissions: Permissions::all(),
    };
    let state =
        EncryptionState::try_from(version).map_err(|e| format!("failed to set up encryption: {e}"))?;
    doc.encrypt(&state)
        .map_err(|e| format!("failed to encrypt PDF: {e}"))?;

    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to write PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    /// Build a minimal one-page PDF.
    fn sample_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content_id = doc.add_object(lopdf::Stream::new(
            dictionary! {},
            b"BT /F1 12 Tf 20 700 Td (hello) Tj ET".to_vec(),
        ));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1i64,
        };
        doc.objects.insert(pages_id, lopdf::Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn encrypts_and_round_trips() {
        let pdf = sample_pdf();
        assert!(!Document::load_mem(&pdf).unwrap().is_encrypted());

        let enc = run(&pdf, "s3cret", "").unwrap();
        let mut doc = Document::load_mem(&enc).unwrap();
        assert!(doc.is_encrypted(), "output should be encrypted");

        // Correct password decrypts.
        doc.decrypt("s3cret").unwrap();

        // Wrong password fails.
        let mut doc2 = Document::load_mem(&enc).unwrap();
        assert!(doc2.decrypt("nope").is_err());
    }

    #[test]
    fn distinct_owner_password() {
        let pdf = sample_pdf();
        let enc = run(&pdf, "user-pw", "owner-pw").unwrap();
        let mut doc = Document::load_mem(&enc).unwrap();
        // Either password opens the document.
        assert!(doc.decrypt("user-pw").is_ok());
        let mut doc2 = Document::load_mem(&enc).unwrap();
        assert!(doc2.decrypt("owner-pw").is_ok());
    }

    #[test]
    fn errors() {
        let pdf = sample_pdf();
        assert!(run(&pdf, "", "").is_err()); // empty password
        assert!(run(b"not a pdf", "pw", "").is_err()); // garbage
        // already-encrypted input is rejected
        let enc = run(&pdf, "pw", "").unwrap();
        assert!(run(&enc, "pw2", "").is_err());
    }
}
