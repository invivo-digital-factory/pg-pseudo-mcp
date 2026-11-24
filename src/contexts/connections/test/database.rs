const DATABASE_URL_VAR: &str = "SCARIF_TEST_DATABASE_URL";

pub fn database_url() -> String {
    std::env::var(DATABASE_URL_VAR)
        .unwrap_or_else(|_| panic!("{DATABASE_URL_VAR} must point at the docker compose database"))
}
