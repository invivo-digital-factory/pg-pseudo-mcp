use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionSource {
    File,
    Dbeaver,
}

impl ConnectionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Dbeaver => "dbeaver",
        }
    }
}
