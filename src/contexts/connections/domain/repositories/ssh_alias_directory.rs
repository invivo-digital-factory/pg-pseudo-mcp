pub trait SshAliasDirectory: Send + Sync {
    fn alias_for(&self, host: &str, port: u16, user: &str) -> Option<String>;

    fn known_aliases(&self) -> Vec<String>;
}
