use super::schema::RawEntry;
use crate::contexts::connections::domain::repositories::AzureSignInTokenProvider;
use crate::contexts::connections::domain::services::is_azure_postgres_host;
use crate::contexts::connections::domain::types::{
    AuthMode, ConnectParams, ConnectionDefinition, ConnectionEntry, ConnectionSource,
    ConnectionStatus, PasswordSource, TunnelParams,
};
use crate::contexts::connections::infrastructure::data_sources::openssh::OpenSshResolver;
use crate::contexts::connections::infrastructure::data_sources::password_resolution::{
    SignIn, pgpass_or_azure_token,
};
use crate::contexts::connections::infrastructure::data_sources::pgpass;

const AZURE_HOST_REASON: &str = "auth = \"azure\" needs an Azure PostgreSQL host (ending in .postgres.database.azure.com): the Azure token is only sent to Azure PostgreSQL hosts";

pub struct FileEvaluator<'a> {
    sign_in: SignIn<'a>,
    resolver: &'a OpenSshResolver,
    database: Option<&'a str>,
}

impl<'a> FileEvaluator<'a> {
    pub fn new(
        tokens: &'a dyn AzureSignInTokenProvider,
        resolver: &'a OpenSshResolver,
        database: Option<&'a str>,
    ) -> Self {
        Self {
            sign_in: SignIn::new(tokens),
            resolver,
            database,
        }
    }

    pub fn entry(&self, raw: &RawEntry) -> ConnectionEntry {
        let (status, params, database, connection_type) = match &raw.definition {
            Err(reason) => (
                ConnectionStatus::Unavailable {
                    reason: reason.clone(),
                },
                None,
                None,
                "unknown".to_string(),
            ),
            Ok(definition) => {
                let database = self
                    .database
                    .map_or_else(|| definition.database.clone(), str::to_string);
                let connection_type = definition.connection_type.as_str().to_string();
                match self.params(definition, &database) {
                    Ok(params) => (
                        ConnectionStatus::Available,
                        Some(params),
                        Some(database),
                        connection_type,
                    ),
                    Err(status) => (status, None, Some(database), connection_type),
                }
            }
        };
        ConnectionEntry {
            id: raw.name.clone(),
            name: raw.name.clone(),
            declared_name: raw.name.clone(),
            source: ConnectionSource::File,
            folder: None,
            project: "file".to_string(),
            engine: "postgresql".to_string(),
            connection_type,
            database,
            status,
            params,
        }
    }

    fn params(
        &self,
        definition: &ConnectionDefinition,
        database: &str,
    ) -> Result<ConnectParams, ConnectionStatus> {
        let target = pgpass::Target {
            host: &definition.host,
            port: definition.port,
            database,
            user: &definition.user,
        };
        let password = match &definition.auth {
            AuthMode::Pgpass => pgpass_or_azure_token(&target, false, &self.sign_in)?,
            AuthMode::Azure => {
                if !is_azure_postgres_host(&definition.host) {
                    return Err(ConnectionStatus::Unavailable {
                        reason: AZURE_HOST_REASON.to_string(),
                    });
                }
                pgpass_or_azure_token(&target, true, &self.sign_in)?
            }
            AuthMode::Command(argv) => PasswordSource::Command(argv.clone()),
        };
        let tunnel = definition
            .ssh_alias
            .as_deref()
            .map(|alias| {
                self.resolver.resolve(alias).map(|hops| TunnelParams {
                    hops,
                    remote_host: definition.host.clone(),
                    remote_port: definition.port,
                })
            })
            .transpose()?;
        let sslmode = password.effective_sslmode(definition.sslmode);
        Ok(ConnectParams {
            host: definition.host.clone(),
            port: definition.port,
            database: database.to_string(),
            user: definition.user.clone(),
            password,
            sslmode,
            tunnel,
        })
    }
}
