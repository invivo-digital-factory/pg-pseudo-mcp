use std::net::SocketAddr;
use std::time::Duration;

use mongodb::options::{
    ClientOptions, ConnectionString, Credential, HostInfo, ReadPreference, SelectionCriteria,
    ServerAddress, ServerMonitoringMode, Socks5Proxy, Tls, TlsOptions,
};

use crate::contexts::connections::domain::types::{MongoConnectParams, MongoTransport, Secret};
use crate::contexts::connections::infrastructure::data_sources::socks::ProxyCredentials;

pub const SELECTION_TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const APP_NAME: &str = "scarif";
const AUTH_SOURCE: &str = "admin";
const MAX_POOL_SIZE: u32 = 2;

pub struct ProxyEndpoint<'a> {
    pub address: SocketAddr,
    pub credentials: &'a ProxyCredentials,
}

pub fn connection_string(params: &MongoConnectParams) -> ConnectionString {
    let mut string = ConnectionString::default();
    let (host_info, tls) = match &params.transport {
        MongoTransport::Srv { host } => (HostInfo::DnsRecord(host.clone()), true),
        MongoTransport::Seed { host, port, tls } => (
            HostInfo::HostIdentifiers(vec![ServerAddress::Tcp {
                host: host.clone(),
                port: Some(*port),
            }]),
            *tls,
        ),
    };
    string.host_info = host_info;
    string.tls = Some(if tls {
        Tls::Enabled(TlsOptions::default())
    } else {
        Tls::Disabled
    });
    string.replica_set.clone_from(&params.replica_set);
    string
}

pub async fn client_options(
    params: &MongoConnectParams,
    password: &Secret,
    proxy: Option<ProxyEndpoint<'_>>,
    selection_timeout: Duration,
) -> Result<ClientOptions, mongodb::error::Error> {
    let mut options = ClientOptions::parse(connection_string(params)).await?;
    let login = Credential::builder()
        .username(params.user.clone())
        .password(password.expose().to_string())
        .source(AUTH_SOURCE.to_string())
        .build();
    options.credential = Some(login);
    options.selection_criteria = Some(SelectionCriteria::ReadPreference(
        ReadPreference::SecondaryPreferred { options: None },
    ));
    options.max_pool_size = Some(MAX_POOL_SIZE);
    options.server_monitoring_mode = Some(ServerMonitoringMode::Poll);
    options.server_selection_timeout = Some(selection_timeout);
    options.connect_timeout = Some(CONNECT_TIMEOUT);
    options.app_name = Some(APP_NAME.to_string());
    options.socks5_proxy = proxy.map(|proxy| {
        Socks5Proxy::builder()
            .host(proxy.address.ip().to_string())
            .port(proxy.address.port())
            .authentication((
                proxy.credentials.username().to_string(),
                proxy.credentials.password().to_string(),
            ))
            .build()
    });
    Ok(options)
}

#[cfg(test)]
mod unit {
    use mongodb::options::{
        HostInfo, ReadPreference, SelectionCriteria, ServerAddress, ServerMonitoringMode, Tls,
    };

    use super::*;
    use crate::contexts::connections::domain::types::MongoTransport;

    fn seed(host: &str, tls: bool) -> MongoConnectParams {
        MongoConnectParams {
            transport: MongoTransport::Seed {
                host: host.into(),
                port: 1024,
                tls,
            },
            replica_set: Some("rs0".into()),
            database: Some("shop".into()),
            user: "atlas-reader".into(),
            password_command: vec!["/bin/echo".into(), "pw".into()],
            ssh_hops: None,
        }
    }

    async fn options_for(params: &MongoConnectParams) -> ClientOptions {
        client_options(
            params,
            &Secret::new("atlas-reader-test-pw"),
            None,
            SELECTION_TIMEOUT,
        )
        .await
        .expect("seed options need no DNS")
    }

    #[tokio::test]
    async fn une_amorce_donne_un_seul_hote_structure_quelle_que_soit_sa_forme() {
        for host in ["pl-0.atlas.scarif.test", "a@b/c", "h1,h2", "h1:27018"] {
            let options = options_for(&seed(host, false)).await;
            assert_eq!(
                options.hosts,
                vec![ServerAddress::Tcp {
                    host: host.into(),
                    port: Some(1024)
                }],
                "{host}"
            );
            assert_eq!(options.default_database, None, "{host}");
        }
    }

    #[tokio::test]
    async fn l_identifiant_est_pris_tel_quel_avec_la_source_admin() {
        let mut params = seed("pl-0.atlas.scarif.test", false);
        params.user = "jane@corp:x/y?authSource=shop".into();
        let options = options_for(&params).await;
        let credential = options.credential.expect("credential");
        assert_eq!(
            credential.username.as_deref(),
            Some("jane@corp:x/y?authSource=shop")
        );
        assert_eq!(credential.password.as_deref(), Some("atlas-reader-test-pw"));
        assert_eq!(credential.source.as_deref(), Some("admin"));
        assert_eq!(credential.mechanism, None);
    }

    #[tokio::test]
    async fn les_options_fixes_de_scarif_sont_posees() {
        let options = options_for(&seed("pl-0.atlas.scarif.test", false)).await;
        assert_eq!(options.app_name.as_deref(), Some("scarif"));
        assert_eq!(options.repl_set_name.as_deref(), Some("rs0"));
        assert_eq!(options.max_pool_size, Some(2));
        assert_eq!(
            options.server_monitoring_mode,
            Some(ServerMonitoringMode::Poll)
        );
        assert_eq!(
            options.server_selection_timeout,
            Some(Duration::from_secs(10))
        );
        assert_eq!(options.connect_timeout, Some(Duration::from_secs(10)));
        assert_eq!(options.direct_connection, None);
        assert_eq!(
            options.selection_criteria,
            Some(SelectionCriteria::ReadPreference(
                ReadPreference::SecondaryPreferred { options: None }
            ))
        );
        assert_eq!(options.socks5_proxy, None);
    }

    #[tokio::test]
    async fn le_tls_d_une_amorce_suit_la_connexion() {
        let plain = options_for(&seed("pl-0.atlas.scarif.test", false)).await;
        assert_eq!(plain.tls, Some(Tls::Disabled));
        let secured = options_for(&seed("pl-0.atlas.scarif.test", true)).await;
        assert!(
            matches!(secured.tls, Some(Tls::Enabled(_))),
            "{:?}",
            secured.tls
        );
    }

    #[tokio::test]
    async fn le_proxy_pointe_sur_le_front_local_avec_ses_identifiants() {
        let credentials = ProxyCredentials::generate().expect("credentials");
        let address: SocketAddr = "127.0.0.1:41234".parse().expect("address");
        let options = client_options(
            &seed("pl-0.atlas.scarif.test", false),
            &Secret::new("pw"),
            Some(ProxyEndpoint {
                address,
                credentials: &credentials,
            }),
            SELECTION_TIMEOUT,
        )
        .await
        .expect("options");
        let proxy = options.socks5_proxy.expect("proxy");
        assert_eq!(proxy.host, "127.0.0.1");
        assert_eq!(proxy.port, Some(41234));
        assert_eq!(
            proxy.authentication,
            Some((
                credentials.username().to_string(),
                credentials.password().to_string()
            ))
        );
    }

    #[test]
    fn une_connexion_srv_passe_par_la_resolution_srv_du_pilote_avec_tls() {
        let params = MongoConnectParams {
            transport: MongoTransport::Srv {
                host: "cluster0.atlas.scarif.test".into(),
            },
            replica_set: None,
            ..seed("unused", false)
        };
        let string = connection_string(&params);
        assert_eq!(
            string.host_info,
            HostInfo::DnsRecord("cluster0.atlas.scarif.test".into())
        );
        assert!(
            matches!(string.tls, Some(Tls::Enabled(_))),
            "{:?}",
            string.tls
        );
        assert_eq!(string.replica_set, None);
    }
}
