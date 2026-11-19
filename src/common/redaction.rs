pub fn redact_home(text: &str) -> String {
    let Some(home) = std::env::var_os("HOME").map(std::path::PathBuf::from) else {
        return text.to_string();
    };
    let home = home.to_string_lossy();
    let home = home.trim_end_matches('/');
    if home.len() < 2 {
        return text.to_string();
    }
    text.replace(home, "~")
}
