//! gizza-ai/basic-auth-header-generator core — pure compute, shared by the chat
//! skill block and the web page. No wafer/wasm-bindgen deps. Builds an HTTP Basic
//! Authorization header value: `Basic base64(username:password)` per RFC 7617.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

/// Build the header. With `full_header=true` returns `Authorization: Basic …`,
/// otherwise just the value `Basic …`. Username may not contain a colon (the
/// `username:password` separator); password may be empty.
pub fn build(username: &str, password: &str, full_header: bool) -> Result<String, String> {
    if username.is_empty() {
        return Err("username is required".into());
    }
    if username.contains(':') {
        return Err("username must not contain a colon ':' (it separates user from password)".into());
    }
    let token = B64.encode(format!("{username}:{password}"));
    let value = format!("Basic {token}");
    Ok(if full_header { format!("Authorization: {value}") } else { value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_value() {
        // base64("aladdin:opensesame") = YWxhZGRpbjpvcGVuc2VzYW1l
        assert_eq!(build("aladdin", "opensesame", false).unwrap(), "Basic YWxhZGRpbjpvcGVuc2VzYW1l");
    }

    #[test]
    fn full_header_form() {
        assert_eq!(build("u", "p", true).unwrap(), "Authorization: Basic dTpw");
    }

    #[test]
    fn empty_password_ok() {
        // base64("user:") = dXNlcjo=
        assert_eq!(build("user", "", false).unwrap(), "Basic dXNlcjo=");
    }

    #[test]
    fn missing_username_errors() {
        assert!(build("", "p", false).is_err());
    }

    #[test]
    fn colon_in_username_errors() {
        assert!(build("a:b", "p", false).is_err());
    }

    #[test]
    fn unicode_password() {
        // round-trips through utf-8 base64
        let v = build("u", "pâss", false).unwrap();
        let b64 = v.strip_prefix("Basic ").unwrap();
        let decoded = B64.decode(b64).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "u:pâss");
    }
}
