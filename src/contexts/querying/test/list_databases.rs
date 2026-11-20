use std::sync::Arc;

use super::fakes::{FakeAnonymizer, FakeDatabase, FakeTranslator, ports};
use crate::contexts::querying::domain::errors::{DatabaseFailure, QueryingError};
use crate::contexts::querying::domain::types::{DatabaseListing, ListDatabasesRequest};
use crate::contexts::querying::use_cases::ListDatabases;

fn request() -> ListDatabasesRequest {
    ListDatabasesRequest {
        connection: "Shop".into(),
    }
}

mod unit {
    use super::*;

    #[tokio::test]
    async fn les_noms_de_bases_sont_rendus_tels_quels_sans_etre_pseudonymises() {
        let listing = DatabaseListing {
            databases: vec!["claire.martin_shop".into(), "postgres".into()],
            default: Some("postgres".into()),
            truncated: false,
        };
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let use_case = ListDatabases::new(
            FakeDatabase::listing(listing.clone()),
            ports(FakeTranslator::default(), Arc::clone(&anonymizer)),
        );

        let response = use_case.execute(request()).await.unwrap();

        assert_eq!(response, listing);
        assert!(anonymizer.injected_seen().is_empty());
    }

    #[tokio::test]
    async fn un_echec_de_connexion_passe_par_la_pseudonymisation_du_texte() {
        let database = FakeDatabase::failing(DatabaseFailure::ConnectFailed {
            connection: "Shop".into(),
            reason: "SQLSTATE 28P01: password authentication failed".into(),
        });
        let anonymizer: Arc<FakeAnonymizer> = Arc::default();
        let use_case = ListDatabases::new(
            database,
            ports(FakeTranslator::default(), Arc::clone(&anonymizer)),
        );

        let error = use_case.execute(request()).await.unwrap_err();

        assert!(matches!(error, QueryingError::ConnectFailed { .. }));
        assert_eq!(anonymizer.injected_seen().len(), 1);
    }

    #[tokio::test]
    async fn un_registre_inutilisable_refuse_avant_d_appeler_la_base() {
        let use_case = ListDatabases::new(
            FakeDatabase::failing(DatabaseFailure::Timeout),
            Err("store is locked".into()),
        );

        let error = use_case.execute(request()).await.unwrap_err();

        assert_eq!(
            error,
            QueryingError::StoreUnavailable {
                reason: "store is locked".into()
            }
        );
    }
}
