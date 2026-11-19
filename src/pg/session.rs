use std::future::poll_fn;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio_postgres::config::SslMode as WireSslMode;
use tokio_postgres::{AsyncMessage, CancelToken, Client, Config, Connection, NoTls, Transaction};
use tokio_postgres_rustls::MakeRustlsConnect;

use super::decode::decode_row;
use super::error::{DbMessage, PgError};
use super::options::{ConnectOptions, SslMode};
use super::output::{ColumnMeta, QueryOutput};
use super::provenance::SourceCache;
use super::tls::verified_connector;

const APPLICATION_NAME: &str = "pg-pseudo-mcp";
const LOCK_TIMEOUT_MS: u64 = 2000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CLIENT_GRACE: Duration = Duration::from_secs(5);
const CANCEL_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_STATEMENT_TIMEOUT_MS: u64 = i32::MAX as u64;
const UNDEFINED_PARAMETER: &str = "42P02";

pub struct Session {
    options: ConnectOptions,
    live: Option<Live>,
    sources: SourceCache,
}

struct Live {
    client: Client,
    notices: UnboundedReceiver<DbMessage>,
    cancel_token: CancelToken,
    tls: Option<MakeRustlsConnect>,
}

pub async fn connect(options: &ConnectOptions) -> Result<Session, PgError> {
    let live = open(options).await?;
    Ok(Session {
        options: options.clone(),
        live: Some(live),
        sources: SourceCache::default(),
    })
}

impl Session {
    pub async fn run_read_only(
        &mut self,
        sql: &str,
        max_rows: usize,
        timeout_ms: u64,
    ) -> Result<QueryOutput, PgError> {
        let timeout_ms = timeout_ms.clamp(1, MAX_STATEMENT_TIMEOUT_MS);
        let live = match &mut self.live {
            Some(live) if !live.client.is_closed() => live,
            slot => slot.insert(open(&self.options).await?),
        };
        live.drain_notices();
        let deadline = Duration::from_millis(timeout_ms) + CLIENT_GRACE;
        let execution = execute(
            &mut live.client,
            &mut self.sources,
            sql,
            max_rows,
            timeout_ms,
        );
        match tokio::time::timeout(deadline, execution).await {
            Ok(Execution { result, healthy }) => {
                let notices = live.drain_notices();
                if !healthy || live.client.is_closed() {
                    self.live = None;
                }
                result.map(|output| QueryOutput { notices, ..output })
            }
            Err(_) => {
                if let Some(live) = self.live.take() {
                    live.abandon();
                }
                Err(PgError::Timeout)
            }
        }
    }
}

struct Execution {
    result: Result<QueryOutput, PgError>,
    healthy: bool,
}

async fn execute(
    client: &mut Client,
    sources: &mut SourceCache,
    sql: &str,
    max_rows: usize,
    timeout_ms: u64,
) -> Execution {
    let transaction = match client.build_transaction().read_only(true).start().await {
        Ok(transaction) => transaction,
        Err(error) => {
            let error = PgError::from_postgres(&error);
            return Execution {
                healthy: !error.breaks_connection(),
                result: Err(error),
            };
        }
    };
    let result = run_statement(&transaction, sources, sql, max_rows, timeout_ms).await;
    let rollback = transaction.rollback().await;
    let healthy = rollback.is_ok() && !result.as_ref().is_err_and(PgError::breaks_connection);
    let result = match (result, rollback) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(PgError::Connect(error.to_string())),
        (Ok(output), Ok(())) => Ok(output),
    };
    Execution { result, healthy }
}

async fn run_statement(
    transaction: &Transaction<'_>,
    sources: &mut SourceCache,
    sql: &str,
    max_rows: usize,
    timeout_ms: u64,
) -> Result<QueryOutput, PgError> {
    let pg = |error: tokio_postgres::Error| PgError::from_postgres(&error);
    transaction
        .batch_execute(&format!(
            "SET LOCAL statement_timeout = {timeout_ms}; SET LOCAL lock_timeout = {LOCK_TIMEOUT_MS}"
        ))
        .await
        .map_err(pg)?;
    let statement = transaction.prepare(sql).await.map_err(pg)?;
    if !statement.params().is_empty() {
        return Err(PgError::Db(DbMessage {
            severity: "ERROR".into(),
            sqlstate: Some(UNDEFINED_PARAMETER.into()),
            message: "parameter placeholders are not supported; inline the values".into(),
            detail: None,
            hint: None,
            where_: None,
        }));
    }
    let sources = sources
        .resolve(transaction, statement.columns())
        .await
        .map_err(pg)?;
    let columns = statement
        .columns()
        .iter()
        .zip(sources)
        .map(|(column, source)| ColumnMeta {
            name: column.name().to_string(),
            type_name: column.type_().name().to_string(),
            type_oid: column.type_().oid(),
            source,
        })
        .collect();
    let portal = transaction.bind(&statement, &[]).await.map_err(pg)?;
    let fetch = i32::try_from(max_rows.saturating_add(1)).unwrap_or(i32::MAX);
    let rows = transaction.query_portal(&portal, fetch).await.map_err(pg)?;
    let more_rows = rows.len() > max_rows;
    let rows = rows
        .iter()
        .take(max_rows)
        .map(decode_row)
        .collect::<Result<Vec<_>, _>>()
        .map_err(PgError::Decode)?;
    Ok(QueryOutput {
        columns,
        rows,
        more_rows,
        notices: Vec::new(),
    })
}

async fn open(options: &ConnectOptions) -> Result<Live, PgError> {
    let mut config = Config::new();
    config
        .host(&options.host)
        .port(options.port)
        .dbname(&options.database)
        .user(&options.user)
        .password(options.password.as_bytes())
        .application_name(APPLICATION_NAME)
        .connect_timeout(CONNECT_TIMEOUT);
    let refused = |error: tokio_postgres::Error| PgError::Connect(error.to_string());
    match options.sslmode {
        SslMode::Disable => {
            config.ssl_mode(WireSslMode::Disable);
            let (client, connection) = config.connect(NoTls).await.map_err(refused)?;
            Ok(Live::start(client, connection, None))
        }
        mode => {
            config.ssl_mode(match mode {
                SslMode::Allow | SslMode::Prefer => WireSslMode::Prefer,
                _ => WireSslMode::Require,
            });
            let tls = verified_connector()
                .map_err(|error| PgError::Connect(format!("tls setup failed: {error}")))?;
            let (client, connection) = config.connect(tls.clone()).await.map_err(refused)?;
            Ok(Live::start(client, connection, Some(tls)))
        }
    }
}

impl Live {
    fn start<S, T>(
        client: Client,
        connection: Connection<S, T>,
        tls: Option<MakeRustlsConnect>,
    ) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (sender, notices) = mpsc::unbounded_channel();
        tokio::spawn(drive(connection, sender));
        Self {
            cancel_token: client.cancel_token(),
            client,
            notices,
            tls,
        }
    }

    fn drain_notices(&mut self) -> Vec<DbMessage> {
        let mut drained = Vec::new();
        while let Ok(notice) = self.notices.try_recv() {
            drained.push(notice);
        }
        drained
    }

    fn abandon(self) {
        let Self {
            client,
            cancel_token,
            tls,
            ..
        } = self;
        tokio::spawn(async move {
            let cancel = async {
                match tls {
                    Some(tls) => cancel_token.cancel_query(tls).await,
                    None => cancel_token.cancel_query(NoTls).await,
                }
            };
            let _ = tokio::time::timeout(CANCEL_TIMEOUT, cancel).await;
            drop(client);
        });
    }
}

async fn drive<S, T>(mut connection: Connection<S, T>, notices: UnboundedSender<DbMessage>)
where
    S: AsyncRead + AsyncWrite + Unpin,
    T: AsyncRead + AsyncWrite + Unpin,
{
    while let Some(Ok(message)) = poll_fn(|cx| connection.poll_message(cx)).await {
        if let AsyncMessage::Notice(notice) = message {
            let _ = notices.send(DbMessage::from_db_error(&notice));
        }
    }
}
