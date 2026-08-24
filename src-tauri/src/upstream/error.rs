use reqwest::StatusCode;

const MAX_RESPONSE_BODY_BYTES: usize = 4096;

pub fn response_body(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_RESPONSE_BODY_BYTES)]).to_string()
}

#[derive(Debug, Clone)]
pub struct UpstreamError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}
