mod common;

use std::str::FromStr;
use std::time::{Duration, Instant};

use pg_pseudo_mcp::pg::{
    Cell, ColumnSource, ConnectOptions, PgError, QueryOutput, Session, SslMode, connect,
};
use tokio_postgres::config::Host;
use tokio_postgres::{Config, NoTls};

const TIMEOUT_MS: u64 = 10_000;

fn options() -> ConnectOptions {
    let config = Config::from_str(&common::database_url()).expect("valid database url");
    let host = match config.get_hosts().first() {
        Some(Host::Tcp(host)) => host.clone(),
        _ => "localhost".to_string(),
    };
    ConnectOptions {
        host,
        port: config.get_ports().first().copied().unwrap_or(5432),
        database: config.get_dbname().unwrap_or("pgpseudo").to_string(),
        user: config.get_user().unwrap_or("pgpseudo").to_string(),
        password: config
            .get_password()
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .unwrap_or_default(),
        sslmode: SslMode::Disable,
    }
}

async fn session() -> Session {
    connect(&options())
        .await
        .expect("connect to the test database")
}

async fn run(session: &mut Session, sql: &str) -> QueryOutput {
    session
        .run_read_only(sql, 100, TIMEOUT_MS)
        .await
        .unwrap_or_else(|error| panic!("{sql} failed: {error}"))
}

async fn single(session: &mut Session, sql: &str) -> Vec<Cell> {
    let output = run(session, sql).await;
    assert_eq!(output.rows.len(), 1, "{sql} must return one row");
    output.rows.into_iter().next().unwrap()
}

fn text(value: &str) -> Cell {
    Cell::Text(value.to_string())
}

fn source(table: &str, column: &str) -> Option<ColumnSource> {
    Some(ColumnSource {
        schema: "public".to_string(),
        table: table.to_string(),
        column: column.to_string(),
    })
}

#[tokio::test]
#[ignore]
async fn seeded_customer_columns_decode_to_typed_cells() {
    let mut session = session().await;
    let output = run(
        &mut session,
        "SELECT created_at, profile, emails_secondaires, avatar, date_naissance, telephone_secours \
         FROM customers WHERE id = 1",
    )
    .await;
    let row = &output.rows[0];
    assert_eq!(row[0], Cell::TimestampTz("2026-01-15T10:00:00Z".into()));
    match &row[1] {
        Cell::Json(value) => {
            assert!(value.is_object());
            assert_eq!(value["contact"]["age"], 42);
        }
        other => panic!("profile decoded as {other:?}"),
    }
    assert_eq!(
        row[2],
        Cell::Array(vec![
            text("ylwenna.k@canary-mail.fr"),
            text("yk.pro@canary-mail.fr")
        ])
    );
    assert_eq!(
        row[3],
        Cell::Opaque {
            type_name: "bytea".into(),
            len: 4
        }
    );
    assert_eq!(row[4], Cell::Date("1985-11-02".into()));
    assert_eq!(row[5], Cell::Int(33641278395));
    assert_eq!(output.columns[0].type_name, "timestamptz");
    assert_eq!(output.columns[2].type_name, "_text");
    assert_eq!(
        output.columns[2].source,
        source("customers", "emails_secondaires")
    );
}

#[tokio::test]
#[ignore]
async fn seeded_order_numeric_and_enum_decode_exactly() {
    let mut session = session().await;
    let row = single(
        &mut session,
        "SELECT amount, status FROM orders WHERE id = 10",
    )
    .await;
    assert_eq!(row[0], Cell::Numeric("12345.6700".into()));
    assert_eq!(row[1], text("paid"));
    let row = single(&mut session, "SELECT amount FROM orders WHERE id = 13").await;
    assert_eq!(row[0], Cell::Numeric("0.0000".into()));
    let row = single(&mut session, "SELECT -0.05::numeric, 'NaN'::numeric").await;
    assert_eq!(
        row,
        vec![Cell::Numeric("-0.05".into()), Cell::Numeric("NaN".into())]
    );
}

#[tokio::test]
#[ignore]
async fn null_columns_decode_to_null() {
    let mut session = session().await;
    let row = single(
        &mut session,
        "SELECT telephone, profile, emails_secondaires, avatar, date_naissance FROM customers WHERE id = 3",
    )
    .await;
    assert!(row.iter().all(|cell| *cell == Cell::Null), "{row:?}");
}

#[tokio::test]
#[ignore]
async fn multi_dimensional_arrays_nest_and_keep_null_elements() {
    let mut session = session().await;
    let row = single(
        &mut session,
        "SELECT ARRAY[[1,2],[3,4]], ARRAY[1,NULL,3], '{}'::int[]",
    )
    .await;
    assert_eq!(
        row[0],
        Cell::Array(vec![
            Cell::Array(vec![Cell::Int(1), Cell::Int(2)]),
            Cell::Array(vec![Cell::Int(3), Cell::Int(4)]),
        ])
    );
    assert_eq!(
        row[1],
        Cell::Array(vec![Cell::Int(1), Cell::Null, Cell::Int(3)])
    );
    assert_eq!(row[2], Cell::Array(vec![]));
}

#[tokio::test]
#[ignore]
async fn whole_row_composites_carry_field_names() {
    let mut session = session().await;
    let output = run(&mut session, "SELECT c FROM customers c WHERE id = 1").await;
    assert_eq!(output.columns[0].source, None);
    let Cell::Composite(fields) = &output.rows[0][0] else {
        panic!("expected a composite, got {:?}", output.rows[0][0]);
    };
    let names: Vec<&str> = fields.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names[..4], ["id", "prenom", "nom", "email"]);
    assert_eq!(fields[0].1, Cell::Int(1));
    assert_eq!(fields[3].1, text("ylwenna.kervadec@canary-mail.fr"));
    assert_eq!(fields[9].1, Cell::Date("1985-11-02".into()));
    assert_eq!(
        fields[13].1,
        Cell::Opaque {
            type_name: "bytea".into(),
            len: 4
        }
    );
    let output = run(&mut session, "SELECT c FROM customers c WHERE id = 3").await;
    let Cell::Composite(fields) = &output.rows[0][0] else {
        panic!("expected a composite, got {:?}", output.rows[0][0]);
    };
    assert_eq!(fields[4], ("telephone".into(), Cell::Null));
}

#[tokio::test]
#[ignore]
async fn anonymous_records_name_fields_by_position() {
    let mut session = session().await;
    let row = single(&mut session, "SELECT ROW(1, 'x'::text, NULL::int)").await;
    assert_eq!(
        row[0],
        Cell::Composite(vec![
            ("f1".into(), Cell::Int(1)),
            ("f2".into(), text("x")),
            ("f3".into(), Cell::Null),
        ])
    );
}

#[tokio::test]
#[ignore]
async fn builtin_scalars_decode_by_oid() {
    let mut session = session().await;
    let row = single(
        &mut session,
        "SELECT true, 1::int2, 2::int4, 3::int8, 'pg_class'::regclass::oid, 1.5::float4, 2.25::float8, \
         'v'::varchar, 'ab'::char(3), 'n'::name, 'a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11'::uuid, \
         '13:45:30.25'::time, '2026-01-15 10:00:00.5'::timestamp, '0044-03-15 BC'::date, \
         '1 year 2 months 3 days 04:05:06.5'::interval, '-1 day -00:00:01'::interval, '0'::interval, \
         '192.168.0.1/24'::inet, '10.0.0.0/8'::cidr, '::1'::inet, '{\"a\": [1, null]}'::json, \
         'infinity'::timestamptz",
    )
    .await;
    assert_eq!(
        row,
        vec![
            Cell::Bool(true),
            Cell::Int(1),
            Cell::Int(2),
            Cell::Int(3),
            Cell::Int(1259),
            Cell::Float(1.5),
            Cell::Float(2.25),
            text("v"),
            text("ab "),
            text("n"),
            Cell::Uuid("a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a11".into()),
            Cell::Time("13:45:30.25".into()),
            Cell::Timestamp("2026-01-15T10:00:00.5".into()),
            Cell::Date("-0043-03-15".into()),
            Cell::Interval("P1Y2M3DT4H5M6.5S".into()),
            Cell::Interval("P-1DT-1S".into()),
            Cell::Interval("PT0S".into()),
            Cell::Inet("192.168.0.1/24".into()),
            Cell::Inet("10.0.0.0/8".into()),
            Cell::Inet("::1".into()),
            Cell::Json(serde_json::json!({"a": [1, null]})),
            Cell::TimestampTz("infinity".into()),
        ]
    );
}

#[tokio::test]
#[ignore]
async fn unsupported_types_are_opaque_with_their_length() {
    let mut session = session().await;
    let row = single(&mut session, "SELECT '[1,5)'::int4range, 'a b'::tsvector").await;
    assert!(matches!(&row[0], Cell::Opaque { type_name, .. } if type_name == "int4range"));
    assert!(matches!(&row[1], Cell::Opaque { type_name, .. } if type_name == "tsvector"));
}

#[tokio::test]
#[ignore]
async fn direct_columns_carry_provenance_and_expressions_do_not() {
    let mut session = session().await;
    let output = run(&mut session, "SELECT email FROM customers").await;
    assert_eq!(output.columns[0].source, source("customers", "email"));
    assert_eq!(output.columns[0].name, "email");
    let output = run(&mut session, "SELECT lower(email) FROM customers").await;
    assert_eq!(output.columns[0].source, None);
}

#[tokio::test]
#[ignore]
async fn subquery_and_cte_columns_resolve_to_the_base_table() {
    let mut session = session().await;
    for sql in [
        "SELECT * FROM (SELECT email FROM customers) s",
        "WITH c AS (SELECT email FROM customers) SELECT email AS e FROM c",
    ] {
        let output = run(&mut session, sql).await;
        assert_eq!(
            output.columns[0].source,
            source("customers", "email"),
            "{sql}"
        );
    }
    let output = run(
        &mut session,
        "SELECT email FROM customers UNION ALL SELECT email FROM customers",
    )
    .await;
    assert_eq!(output.columns[0].source, None);
}

#[tokio::test]
#[ignore]
async fn view_columns_are_reported_against_the_view_by_the_server() {
    let (client, connection) = tokio_postgres::connect(&common::database_url(), NoTls)
        .await
        .unwrap();
    tokio::spawn(connection);
    client
        .batch_execute(
            "CREATE TEMP VIEW v_customers AS SELECT email, lower(nom) AS nom_l FROM customers",
        )
        .await
        .unwrap();
    let view_oid: u32 = client
        .query_one("SELECT 'v_customers'::regclass::oid", &[])
        .await
        .unwrap()
        .get(0);
    let statement = client.prepare("SELECT * FROM v_customers").await.unwrap();
    let origins: Vec<_> = statement
        .columns()
        .iter()
        .map(|column| (column.table_oid(), column.column_id()))
        .collect();
    assert_eq!(
        origins,
        vec![(Some(view_oid), Some(1)), (Some(view_oid), Some(2))]
    );
}

#[tokio::test]
#[ignore]
async fn writes_are_refused_by_the_read_only_transaction() {
    let mut session = session().await;
    let error = session
        .run_read_only("DELETE FROM orders", 100, TIMEOUT_MS)
        .await
        .expect_err("DELETE must fail");
    match error {
        PgError::Db(message) => assert_eq!(message.sqlstate.as_deref(), Some("25006")),
        other => panic!("expected a read-only violation, got {other:?}"),
    }
    let row = single(&mut session, "SELECT count(*) FROM orders").await;
    assert_eq!(row[0], Cell::Int(4));
}

#[tokio::test]
#[ignore]
async fn statement_timeout_is_reported_and_the_session_recovers() {
    let mut session = session().await;
    let started = Instant::now();
    let error = session
        .run_read_only("SELECT pg_sleep(5)", 10, 1000)
        .await
        .expect_err("pg_sleep must time out");
    assert_eq!(error, PgError::Timeout);
    assert!(started.elapsed() < Duration::from_secs(4));
    let row = single(&mut session, "SELECT 1").await;
    assert_eq!(row[0], Cell::Int(1));
}

#[tokio::test]
#[ignore]
async fn row_limit_stops_at_the_limit_and_flags_more_rows() {
    let mut session = session().await;
    let output = session
        .run_read_only("SELECT * FROM big_numbers", 50, TIMEOUT_MS)
        .await
        .unwrap();
    assert_eq!(output.rows.len(), 50);
    assert!(output.more_rows);
    let output = session
        .run_read_only("SELECT * FROM big_numbers WHERE id <= 50", 50, TIMEOUT_MS)
        .await
        .unwrap();
    assert_eq!(output.rows.len(), 50);
    assert!(!output.more_rows);
}

#[tokio::test]
#[ignore]
async fn row_limit_stops_the_server_instead_of_draining_the_result() {
    let mut session = session().await;
    let started = Instant::now();
    let output = session
        .run_read_only("SELECT generate_series(1, 50000000)", 50, 30_000)
        .await
        .unwrap();
    let elapsed = started.elapsed();
    assert_eq!(output.rows.len(), 50);
    assert!(output.more_rows);
    assert!(elapsed < Duration::from_secs(2), "took {elapsed:?}");
}

#[tokio::test]
#[ignore]
async fn server_warnings_are_collected_as_notices() {
    let mut session = session().await;
    let output = run(&mut session, "SELECT pg_advisory_unlock(424242)").await;
    assert_eq!(output.rows[0][0], Cell::Bool(false));
    assert_eq!(output.notices.len(), 1, "{:?}", output.notices);
    assert_eq!(output.notices[0].severity, "WARNING");
    let output = run(&mut session, "SELECT 1").await;
    assert!(output.notices.is_empty());
}

#[tokio::test]
#[ignore]
async fn parameter_placeholders_are_rejected_without_panicking() {
    let mut session = session().await;
    let error = session
        .run_read_only("SELECT $1::int", 10, TIMEOUT_MS)
        .await
        .expect_err("placeholders have no values");
    assert!(matches!(error, PgError::Db(_)), "{error:?}");
    let row = single(&mut session, "SELECT 2").await;
    assert_eq!(row[0], Cell::Int(2));
}

#[tokio::test]
#[ignore]
async fn wrong_password_fails_without_echoing_it() {
    let mut options = options();
    options.password = "definitely-not-the-password".into();
    let error = match connect(&options).await {
        Ok(_) => panic!("connection must be refused"),
        Err(error) => error,
    };
    assert!(matches!(error, PgError::Connect(_)), "{error:?}");
    assert!(!error.to_string().contains("definitely-not-the-password"));
}

#[tokio::test]
#[ignore]
async fn prefer_falls_back_to_plain_and_require_refuses_a_server_without_tls() {
    let mut options = options();
    options.sslmode = SslMode::Prefer;
    let mut session = connect(&options).await.expect("prefer falls back");
    let row = single(&mut session, "SELECT 3").await;
    assert_eq!(row[0], Cell::Int(3));
    options.sslmode = SslMode::Require;
    assert!(matches!(connect(&options).await, Err(PgError::Connect(_))));
}
