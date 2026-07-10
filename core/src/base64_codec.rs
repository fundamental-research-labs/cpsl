//! Shared dependency-free Base64 encoding for byte-oriented sandbox APIs.

const ENCODE_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub(crate) fn encode(data: &[u8]) -> String {
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(ENCODE_TABLE[((triple >> 18) & 0x3f) as usize] as char);
        result.push(ENCODE_TABLE[((triple >> 12) & 0x3f) as usize] as char);
        result.push(if chunk.len() > 1 {
            ENCODE_TABLE[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            ENCODE_TABLE[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn encodes_padding_and_binary_data() {
        assert_eq!(encode(b""), "");
        assert_eq!(encode(b"a"), "YQ==");
        assert_eq!(encode(b"ab"), "YWI=");
        assert_eq!(encode(&[0, 1, 2, 255]), "AAEC/w==");
    }
}
