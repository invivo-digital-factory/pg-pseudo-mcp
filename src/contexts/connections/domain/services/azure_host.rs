const AZURE_POSTGRES_SUFFIX: &str = ".postgres.database.azure.com";

pub fn is_azure_postgres_host(host: &str) -> bool {
    host.trim_end_matches('.')
        .to_ascii_lowercase()
        .ends_with(AZURE_POSTGRES_SUFFIX)
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn un_serveur_postgres_azure_est_reconnu_y_compris_en_acces_prive() {
        assert!(is_azure_postgres_host("shop.postgres.database.azure.com"));
        assert!(is_azure_postgres_host(
            "shop.privatelink.postgres.database.azure.com"
        ));
        assert!(is_azure_postgres_host("Shop.Postgres.Database.Azure.COM."));
    }

    #[test]
    fn un_autre_hote_n_est_pas_un_serveur_postgres_azure() {
        assert!(!is_azure_postgres_host("localhost"));
        assert!(!is_azure_postgres_host("db.internal.example"));
        assert!(!is_azure_postgres_host("postgres.database.azure.com"));
        assert!(!is_azure_postgres_host(
            "shop.postgres.database.azure.com.evil.example"
        ));
        assert!(!is_azure_postgres_host("shop.mysql.database.azure.com"));
    }
}
