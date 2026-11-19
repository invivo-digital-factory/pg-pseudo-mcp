use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use fake::locales::{Data, FR_FR};

use super::dictionary::fold;
use crate::kind::Kind;

const PARTICLES: &[&str] = &[
    "da", "de", "del", "des", "di", "do", "dos", "du", "la", "le",
];

const AMBIGUOUS: &[&str] = &[
    "aime",
    "aimee",
    "amour",
    "ange",
    "aube",
    "aurore",
    "avril",
    "barbe",
    "baron",
    "barre",
    "bataille",
    "beau",
    "blanc",
    "blanche",
    "bois",
    "boisson",
    "bon",
    "bonhomme",
    "bontemps",
    "bouche",
    "boucher",
    "bouillon",
    "boulanger",
    "bouquet",
    "bourgeois",
    "breton",
    "brun",
    "buisson",
    "bureau",
    "cadet",
    "capucine",
    "carre",
    "carriere",
    "celeste",
    "champion",
    "charbonnier",
    "charpentier",
    "chateau",
    "chevalier",
    "chretien",
    "claire",
    "clementine",
    "clerc",
    "colombe",
    "combe",
    "comte",
    "constance",
    "constant",
    "conte",
    "cordier",
    "cornet",
    "cornu",
    "cousin",
    "couturier",
    "desire",
    "eden",
    "esperance",
    "eve",
    "felicite",
    "fleur",
    "forest",
    "forestier",
    "fort",
    "fortune",
    "france",
    "gay",
    "grand",
    "grange",
    "gras",
    "gros",
    "guy",
    "hue",
    "hyacinthe",
    "innocent",
    "iris",
    "jacinthe",
    "jade",
    "jasmin",
    "jasmine",
    "jean",
    "joy",
    "juste",
    "lang",
    "line",
    "lou",
    "loup",
    "luce",
    "maire",
    "maitre",
    "marchand",
    "marechal",
    "marguerite",
    "marie",
    "marin",
    "marine",
    "mas",
    "masse",
    "melodie",
    "merle",
    "meunier",
    "modeste",
    "moine",
    "montagne",
    "moulin",
    "mouton",
    "neveu",
    "nine",
    "noel",
    "normand",
    "olive",
    "page",
    "pages",
    "pain",
    "parent",
    "paris",
    "perle",
    "petit",
    "pierre",
    "poisson",
    "pommier",
    "porte",
    "portier",
    "potier",
    "poulain",
    "poulet",
    "prieur",
    "prudence",
    "reine",
    "renard",
    "roche",
    "rocher",
    "roman",
    "rose",
    "roux",
    "roy",
    "sage",
    "salles",
    "sauvage",
    "sergent",
    "serre",
    "serres",
    "tavernier",
    "terrier",
    "urbain",
    "vacher",
    "vaillant",
    "verger",
    "victoire",
    "vigneron",
    "villain",
    "violette",
    "vivier",
    "voisin",
    "zephyr",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NameHit {
    pub kind: Kind,
    pub first_name: bool,
}

#[derive(Debug, Default, Clone, Copy)]
struct Roles {
    last: bool,
    first: bool,
}

impl Roles {
    fn union(self, other: Roles) -> Roles {
        Roles {
            last: self.last || other.last,
            first: self.first || other.first,
        }
    }

    fn hit(self) -> Option<NameHit> {
        if !self.last && !self.first {
            return None;
        }
        Some(NameHit {
            kind: if self.last {
                Kind::LastName
            } else {
                Kind::FirstName
            },
            first_name: self.first,
        })
    }
}

struct Gazetteer {
    names: HashMap<String, Roles>,
    ambiguous: HashSet<&'static str>,
}

fn gazetteer() -> &'static Gazetteer {
    static GAZETTEER: OnceLock<Gazetteer> = OnceLock::new();
    GAZETTEER.get_or_init(|| {
        let mut names = HashMap::new();
        for entry in <FR_FR as Data>::NAME_LAST_NAME {
            for word in entry.split_whitespace() {
                if let Some(roles) = slot(&mut names, word) {
                    roles.last = true;
                }
            }
        }
        for entry in <FR_FR as Data>::NAME_FIRST_NAME {
            if let Some(roles) = slot(&mut names, entry) {
                roles.first = true;
            }
        }
        Gazetteer {
            names,
            ambiguous: AMBIGUOUS.iter().copied().collect(),
        }
    })
}

fn slot<'a>(names: &'a mut HashMap<String, Roles>, word: &str) -> Option<&'a mut Roles> {
    let folded = fold(word);
    if folded.chars().count() < 3 || PARTICLES.contains(&folded.as_str()) {
        return None;
    }
    Some(names.entry(folded).or_default())
}

pub(crate) fn starts_upper(word: &str) -> bool {
    word.chars().next().is_some_and(char::is_uppercase)
}

fn lookup(word: &str) -> Roles {
    let gazetteer = gazetteer();
    let folded = fold(word);
    if !starts_upper(word) && gazetteer.ambiguous.contains(folded.as_str()) {
        return Roles::default();
    }
    gazetteer.names.get(&folded).copied().unwrap_or_default()
}

pub(crate) fn name_hit(word: &str) -> Option<NameHit> {
    lookup(word).hit().or_else(|| {
        word.split('-')
            .map(lookup)
            .fold(Roles::default(), Roles::union)
            .hit()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_kind(word: &str) -> Option<Kind> {
        name_hit(word).map(|hit| hit.kind)
    }

    #[test]
    fn lookup_ignores_case_and_accents() {
        assert_eq!(name_kind("DUPONT"), Some(Kind::LastName));
        assert_eq!(name_kind("dupont"), Some(Kind::LastName));
        assert_eq!(name_kind("Lefèvre"), Some(Kind::LastName));
        assert_eq!(name_kind("Zoé"), Some(Kind::FirstName));
        assert_eq!(name_kind("zoe"), Some(Kind::FirstName));
    }

    #[test]
    fn ambiguous_words_match_only_when_capitalised() {
        assert_eq!(name_kind("petit"), None);
        assert_eq!(name_kind("Petit"), Some(Kind::LastName));
    }

    #[test]
    fn hyphenated_names_match_through_their_parts() {
        assert!(name_hit("Jean-Luc").is_some_and(|hit| hit.first_name));
        assert_eq!(name_kind("Saint-Martin"), Some(Kind::LastName));
    }

    #[test]
    fn short_entries_and_particles_are_left_out() {
        assert_eq!(name_kind("Le"), None);
        assert_eq!(name_kind("Ly"), None);
        assert_eq!(name_kind("Gall"), Some(Kind::LastName));
    }
}
