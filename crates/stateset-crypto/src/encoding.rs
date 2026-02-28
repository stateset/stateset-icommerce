use crate::CryptoError;

/// Encode u32 as big-endian bytes
#[must_use]
pub const fn u32_be(n: u32) -> [u8; 4] {
    n.to_be_bytes()
}

/// Encode u64 as big-endian bytes
#[must_use]
pub const fn u64_be(n: u64) -> [u8; 8] {
    n.to_be_bytes()
}

/// Encode string with length prefix (VES `ENC_STR`)
#[must_use]
pub fn encode_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut result = Vec::with_capacity(4 + bytes.len());
    result.extend_from_slice(&u32_be(bytes.len() as u32));
    result.extend_from_slice(bytes);
    result
}

/// Convert UUID string to 16-byte array
///
/// # Errors
///
/// Returns [`CryptoError::InvalidUuid`] if the UUID string is not a valid
/// 32-hex-digit UUID (with or without dashes).
pub fn uuid_to_bytes(uuid: &str) -> Result<[u8; 16], CryptoError> {
    let hex_str: String = uuid.chars().filter(|c| *c != '-').collect();
    if hex_str.len() != 32 {
        return Err(CryptoError::InvalidUuid(uuid.to_string()));
    }
    let mut bytes = [0u8; 16];
    hex::decode_to_slice(&hex_str, &mut bytes)
        .map_err(|_| CryptoError::InvalidUuid(uuid.to_string()))?;
    Ok(bytes)
}

/// Convert hex string (with or without 0x prefix) to bytes
///
/// # Errors
///
/// Returns [`CryptoError::InvalidHex`] if the hex string is invalid.
pub fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, CryptoError> {
    let hex_str = hex.strip_prefix("0x").unwrap_or(hex);
    hex::decode(hex_str).map_err(|e| CryptoError::InvalidHex(e.to_string()))
}

/// Convert bytes to hex string with 0x prefix
#[must_use]
pub fn bytes_to_hex(bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u32_be_zero() {
        assert_eq!(u32_be(0), [0, 0, 0, 0]);
    }

    #[test]
    fn u32_be_one() {
        assert_eq!(u32_be(1), [0, 0, 0, 1]);
    }

    #[test]
    fn u32_be_max() {
        assert_eq!(u32_be(u32::MAX), [0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn u64_be_zero() {
        assert_eq!(u64_be(0), [0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn u64_be_one() {
        assert_eq!(u64_be(1), [0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn encode_string_empty() {
        assert_eq!(encode_string(""), vec![0, 0, 0, 0]);
    }

    #[test]
    fn encode_string_hello() {
        let result = encode_string("hello");
        assert_eq!(&result[..4], &[0, 0, 0, 5]);
        assert_eq!(&result[4..], b"hello");
    }

    #[test]
    fn uuid_to_bytes_valid() {
        let bytes = uuid_to_bytes("550e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_eq!(bytes[0], 0x55);
        assert_eq!(bytes.len(), 16);
    }

    #[test]
    fn uuid_to_bytes_invalid_length() {
        assert!(uuid_to_bytes("not-a-uuid").is_err());
    }

    #[test]
    fn hex_to_bytes_with_prefix() {
        let bytes = hex_to_bytes("0xdeadbeef").unwrap();
        assert_eq!(bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn hex_to_bytes_without_prefix() {
        let bytes = hex_to_bytes("deadbeef").unwrap();
        assert_eq!(bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn bytes_to_hex_roundtrip() {
        let original = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let hex_str = bytes_to_hex(&original);
        assert_eq!(hex_str, "0xdeadbeef");
        let back = hex_to_bytes(&hex_str).unwrap();
        assert_eq!(back, original);
    }
}
