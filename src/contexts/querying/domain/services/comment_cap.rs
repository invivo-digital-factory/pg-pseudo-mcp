use crate::contexts::querying::domain::constants::catalog::MAX_COMMENT_CHARS;

pub fn cap_comment(scrubbed: String) -> String {
    let length = scrubbed.chars().count();
    if length <= MAX_COMMENT_CHARS {
        return scrubbed;
    }
    let kept: String = scrubbed.chars().take(MAX_COMMENT_CHARS).collect();
    format!("{kept}…[+{} chars]", length - MAX_COMMENT_CHARS)
}
