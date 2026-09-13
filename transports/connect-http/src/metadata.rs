use crate::framing;
use arut_rpc::Metadata;
use base64::{Engine, engine::general_purpose::STANDARD};

pub(crate) fn encode_value(key: &str, value: &[u8]) -> String {
    if key.ends_with("-bin") {
        STANDARD.encode(value)
    } else {
        String::from_utf8_lossy(value).into_owned()
    }
}

pub(crate) fn decode(headers: &axum::http::HeaderMap) -> Metadata {
    let mut result = Metadata::default();
    for (key, value) in headers {
        let bytes = if key.as_str().ends_with("-bin") {
            framing::decode_binary(value.as_bytes()).unwrap_or_default()
        } else {
            value.as_bytes().to_vec()
        };
        result.insert(key.as_str(), bytes);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_metadata_accepts_padded_and_unpadded_values() {
        for encoded in ["AQI=", "AQI"] {
            let mut headers = axum::http::HeaderMap::new();
            headers.insert("test-bin", encoded.parse().unwrap());
            assert_eq!(decode(&headers).get("test-bin"), Some(&[1, 2][..]));
        }
    }
}
