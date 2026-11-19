#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GeneratorConfig {
    pub phone_pool_limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pseudonym {
    pub text: String,
    pub reversible: bool,
    pub exhausted: bool,
}
