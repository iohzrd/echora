pub mod db;
pub mod error;
pub mod http;
pub mod password;
pub mod validation;

pub use error::*;

pub fn truncate_string(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}
