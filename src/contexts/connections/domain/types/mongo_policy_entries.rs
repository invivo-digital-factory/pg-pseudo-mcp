#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MongoPolicyEntries {
    pub clear: Vec<(String, String)>,
    pub personal: Vec<(String, String, String)>,
}
