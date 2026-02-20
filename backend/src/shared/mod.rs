pub mod db;
pub mod error;
pub mod http;
pub mod password;
pub mod validation;

pub use error::*;

pub fn truncate_string(s: &str, max_chars: usize) -> String {
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => format!("{}...", &s[..byte_idx]),
        None => s.to_string(),
    }
}
