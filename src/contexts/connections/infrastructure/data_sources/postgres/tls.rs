use std::sync::Arc;

use rustls::{ClientConfig, RootCertStore};
use tokio_postgres_rustls::MakeRustlsConnect;

pub(crate) fn verified_connector() -> Result<MakeRustlsConnect, rustls::Error> {
    let roots = RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(roots)
            .with_no_client_auth();
    Ok(MakeRustlsConnect::new(config))
}

#[cfg(test)]
mod unit {
    use super::*;

    #[test]
    fn le_connecteur_verificateur_se_construit_sans_fournisseur_par_defaut() {
        assert!(verified_connector().is_ok());
    }
}
