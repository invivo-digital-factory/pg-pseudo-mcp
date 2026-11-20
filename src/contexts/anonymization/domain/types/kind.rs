use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Email,
    Phone,
    Iban,
    Siren,
    Siret,
    Nir,
    CardNumber,
    IpAddress,
    PersonName,
    LastName,
    FirstName,
    Address,
    City,
    PostalCode,
    Civility,
    Gender,
    BirthDate,
}

impl Kind {
    pub const ALL: [Kind; 17] = [
        Kind::Email,
        Kind::Phone,
        Kind::Iban,
        Kind::Siren,
        Kind::Siret,
        Kind::Nir,
        Kind::CardNumber,
        Kind::IpAddress,
        Kind::PersonName,
        Kind::LastName,
        Kind::FirstName,
        Kind::Address,
        Kind::City,
        Kind::PostalCode,
        Kind::Civility,
        Kind::Gender,
        Kind::BirthDate,
    ];

    pub fn is_reversible(self) -> bool {
        matches!(
            self,
            Kind::Email
                | Kind::Phone
                | Kind::Iban
                | Kind::Siren
                | Kind::Siret
                | Kind::Nir
                | Kind::CardNumber
                | Kind::IpAddress
                | Kind::PersonName
                | Kind::LastName
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Email => "email",
            Kind::Phone => "phone",
            Kind::Iban => "iban",
            Kind::Siren => "siren",
            Kind::Siret => "siret",
            Kind::Nir => "nir",
            Kind::CardNumber => "card_number",
            Kind::IpAddress => "ip_address",
            Kind::PersonName => "person_name",
            Kind::LastName => "last_name",
            Kind::FirstName => "first_name",
            Kind::Address => "address",
            Kind::City => "city",
            Kind::PostalCode => "postal_code",
            Kind::Civility => "civility",
            Kind::Gender => "gender",
            Kind::BirthDate => "birth_date",
        }
    }

    pub fn parse(name: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn les_noms_de_type_font_l_aller_retour_pour_chaque_type() {
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn les_types_a_petit_pool_ne_sont_pas_reversibles() {
        for kind in [
            Kind::FirstName,
            Kind::City,
            Kind::PostalCode,
            Kind::Civility,
            Kind::Gender,
            Kind::BirthDate,
            Kind::Address,
        ] {
            assert!(!kind.is_reversible(), "{kind:?}");
        }
        assert!(Kind::Email.is_reversible());
        assert!(Kind::PersonName.is_reversible());
    }
}
