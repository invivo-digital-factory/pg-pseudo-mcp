mod common;

use common::{CONNECTION, Harness, QueryArgs, is_error, query, text_of};
use serde_json::Value;

const ENTRA_USER: &str = "marie.dupont@yourcompany.fr";
const WRONG_PASSWORD: &str = "hunter2-wrong";

async fn refused_login(user: &str) -> String {
    let harness = Harness::with_login(user, WRONG_PASSWORD);
    let result = query(
        &harness.state,
        QueryArgs {
            connection: CONNECTION.to_string(),
            sql: "SELECT 1".to_string(),
            ..QueryArgs::default()
        },
    )
    .await;
    assert!(is_error(&result));
    text_of(&result)
}

mod e2e {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn un_refus_d_authentification_rend_le_code_et_le_message_du_serveur() {
        let text = refused_login("pgpseudo").await;
        assert!(text.contains("28P01"), "{text}");
        assert!(text.contains("password authentication failed"), "{text}");
        assert!(!text.contains("pgpseudo"), "{text}");
        assert!(!text.contains(WRONG_PASSWORD), "{text}");
    }

    #[tokio::test]
    #[ignore]
    async fn le_nom_d_utilisateur_d_un_refus_d_authentification_est_masque() {
        let text = refused_login(ENTRA_USER).await;
        let body: Value = serde_json::from_str(&text).expect("json error body");
        assert!(text.contains("password authentication failed"), "{text}");
        assert!(!text.contains(ENTRA_USER), "{text}");
        assert!(!text.contains("yourcompany"), "{text}");
        assert!(body.is_object(), "{text}");
        assert!(!text.contains(WRONG_PASSWORD), "{text}");
    }
}
