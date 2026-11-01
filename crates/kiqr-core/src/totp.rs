//! One-time codes (RFC 6238, the kind an authenticator app shows) from a
//! secret the user saved with a login, so the sign-in runner can type the
//! second factor by itself. Only the runner ever calls this; the secret and
//! the code never reach the model.

use hmac::{Hmac, Mac};
use sha1::Sha1;

/// The six-digit code for now, from a base32 secret as sites show it
/// (spaces and case do not matter).
pub fn code(secret: &str) -> Option<String> {
    code_at(secret, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).ok()?.as_secs())
}

pub fn code_at(secret: &str, unix_secs: u64) -> Option<String> {
    let cleaned: String = secret.chars().filter(|c| !c.is_whitespace() && *c != '-').collect::<String>().to_ascii_uppercase();
    let key = base32::decode(base32::Alphabet::Rfc4648 { padding: false }, cleaned.trim_end_matches('='))?;
    let counter = unix_secs / 30;
    let mut mac = Hmac::<Sha1>::new_from_slice(&key).ok()?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let bin = ((digest[offset] as u32 & 0x7f) << 24) | ((digest[offset + 1] as u32) << 16) | ((digest[offset + 2] as u32) << 8) | digest[offset + 3] as u32;
    Some(format!("{:06}", bin % 1_000_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc_6238_vectors() {
        // The RFC's SHA-1 test secret ("12345678901234567890") in base32, and its 8-digit vectors cut to 6.
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        assert_eq!(code_at(secret, 59).as_deref(), Some("287082"));
        assert_eq!(code_at(secret, 1111111109).as_deref(), Some("081804"));
        assert_eq!(code_at(secret, 1234567890).as_deref(), Some("005924"));
        assert_eq!(code_at("gezd gnbv gy3t qojq gezd gnbv gy3t qojq", 59).as_deref(), Some("287082"), "spaces and case are ignored");
        assert!(code_at("not base32!", 59).is_none());
    }
}
