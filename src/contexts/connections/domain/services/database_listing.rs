use crate::contexts::connections::domain::errors::SessionError;
use crate::contexts::connections::domain::types::{Cell, DatabaseListing, QueryOutput};

pub const LIST_DATABASES_SQL: &str = "SELECT datname::text, current_database()::text \
     FROM pg_database WHERE datallowconn AND NOT datistemplate ORDER BY datname";

pub fn read_database_listing(output: &QueryOutput) -> Result<DatabaseListing, SessionError> {
    let mut databases = Vec::with_capacity(output.rows.len());
    let mut default = None;
    for row in &output.rows {
        match row.as_slice() {
            [Cell::Text(name), Cell::Text(current)] => {
                databases.push(name.clone());
                default.get_or_insert_with(|| current.clone());
            }
            _ => {
                return Err(SessionError::Decode(
                    "the database listing has an unexpected shape".to_string(),
                ));
            }
        }
    }
    Ok(DatabaseListing {
        databases,
        default,
        truncated: output.more_rows,
    })
}

#[cfg(test)]
mod unit {
    use super::*;

    fn output(rows: Vec<Vec<Cell>>, more_rows: bool) -> QueryOutput {
        QueryOutput {
            columns: Vec::new(),
            rows,
            more_rows,
            notices: Vec::new(),
        }
    }

    fn row(name: &str, current: &str) -> Vec<Cell> {
        vec![Cell::Text(name.into()), Cell::Text(current.into())]
    }

    #[test]
    fn les_noms_de_bases_sont_rendus_tels_quels_avec_la_base_de_la_connexion() {
        let listing = read_database_listing(&output(
            vec![row("app", "postgres"), row("postgres", "postgres")],
            false,
        ))
        .unwrap();
        assert_eq!(listing.databases, ["app", "postgres"]);
        assert_eq!(listing.default.as_deref(), Some("postgres"));
        assert!(!listing.truncated);
    }

    #[test]
    fn une_liste_coupee_est_signalee_comme_tronquee() {
        let listing = read_database_listing(&output(vec![row("app", "app")], true)).unwrap();
        assert!(listing.truncated);
    }

    #[test]
    fn une_ligne_de_forme_inattendue_est_refusee() {
        let error = read_database_listing(&output(vec![vec![Cell::Int(1)]], false)).unwrap_err();
        assert!(matches!(error, SessionError::Decode(_)));
    }
}
