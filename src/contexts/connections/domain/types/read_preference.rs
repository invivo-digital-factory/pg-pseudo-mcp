#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadPreferenceMode {
    Primary,
    PrimaryPreferred,
    Secondary,
    SecondaryPreferred,
    Nearest,
}

impl ReadPreferenceMode {
    pub const NAMES: [&'static str; 5] = [
        "primary",
        "primaryPreferred",
        "secondary",
        "secondaryPreferred",
        "nearest",
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::PrimaryPreferred => "primaryPreferred",
            Self::Secondary => "secondary",
            Self::SecondaryPreferred => "secondaryPreferred",
            Self::Nearest => "nearest",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "primary" => Some(Self::Primary),
            "primaryPreferred" => Some(Self::PrimaryPreferred),
            "secondary" => Some(Self::Secondary),
            "secondaryPreferred" => Some(Self::SecondaryPreferred),
            "nearest" => Some(Self::Nearest),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MongoReadPreference {
    pub mode: ReadPreferenceMode,
    pub tags: Vec<(String, String)>,
}

impl MongoReadPreference {
    pub fn of(mode: ReadPreferenceMode) -> Self {
        Self {
            mode,
            tags: Vec::new(),
        }
    }
}

impl Default for MongoReadPreference {
    fn default() -> Self {
        Self::of(ReadPreferenceMode::SecondaryPreferred)
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn chaque_mode_se_relit_depuis_son_nom() {
        for name in ReadPreferenceMode::NAMES {
            let mode = ReadPreferenceMode::from_name(name).expect(name);
            assert_eq!(mode.as_str(), name);
        }
        assert_eq!(ReadPreferenceMode::from_name("Secondary"), None);
    }

    #[test]
    fn la_preference_par_defaut_reste_un_secondaire_de_preference() {
        assert_eq!(
            MongoReadPreference::default().mode,
            ReadPreferenceMode::SecondaryPreferred
        );
    }
}
