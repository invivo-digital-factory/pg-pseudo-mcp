#![allow(dead_code)]

pub const DATABASE_URL_VAR: &str = "PG_PSEUDO_TEST_DATABASE_URL";

pub const CANARY_VALUES: &[&str] = &[
    "Ilona",
    "Kervadec",
    "ilona.kervadec@canary-mail.fr",
    "06 41 27 83 95",
    "33641278395",
    "FR76 3000 4000 0312 3456 7890 143",
    "418273652",
    "2851175123456 22",
    "o.thaumassin@canary-mail.fr",
    "kervadec.ilona@canary-mail.fr",
    "ilona.k@canary-mail.fr",
    "ik.pro@canary-mail.fr",
    "Odilon",
    "Thaumassin",
    "odilon.thaumassin@canary-mail.fr",
    "07 58 14 92 36",
    "33758149236",
    "FR14 2004 1010 0505 0001 3M02 606",
    "odilon.t@canary-mail.fr",
    "Maëlys",
    "Brizolier",
    "maelys.brizolier@canary-mail.fr",
    "Ilona Kervadec",
    "Odilon Thaumassin",
    "06 52 18 74 30",
    "12 rue des Lilas",
];

pub fn database_url() -> String {
    std::env::var(DATABASE_URL_VAR)
        .unwrap_or_else(|_| panic!("{DATABASE_URL_VAR} must point at the docker compose database"))
}
