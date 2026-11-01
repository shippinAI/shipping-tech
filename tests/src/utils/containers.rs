

use std::time::{Duration, Instant};

use testcontainers::{
    ContainerAsync, GenericBuildableImage, GenericImage, ImageExt, ReuseDirective,
    core::{CmdWaitFor, ExecCommand, Host, IntoContainerPort, WaitFor},
    runners::{AsyncBuilder, AsyncRunner},
};
use tokio::{net::TcpStream, sync::OnceCell};

const ACME_NETWORK: &str = "shippin.tech-test-acme";

const READY_TIMEOUT: Duration = Duration::from_secs(180);

static FOUNDATIONDB: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static POSTGRES: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static MYSQL: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static MARIADB: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static REDIS: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static NATS: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static MINIO: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static OPENSEARCH: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static MEILISEARCH: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static KEYCLOAK: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static OPENLDAP: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static CHALLTESTSRV: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static PEBBLE: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static POWERDNS: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();
static SCIM_TESTER: OnceCell<ContainerAsync<GenericImage>> = OnceCell::const_new();

const OPENLDAP_LDAPI_URL: &str = "ldapi://%2Fvar%2Frun%2Fslapd%2Fldapi/";

const OPENLDAP_ALLOW_UNAUTHENTICATED_BIND: &str = r#"set -e
ldapmodify -Y EXTERNAL -Q -H "$LDAPI" <<'EOF'
dn: cn=config
changetype: modify
replace: olcAllows
olcAllows: bind_anon_dn
EOF
test "$(ldapwhoami -x -H "$LDAPI" -D uid=john.doe,ou=users,dc=shippin.tech,dc=test -w '')" = anonymous
"#;

const POWERDNS_ZONE_INIT: &str = r#"set -e
for i in $(seq 1 60); do
    pdnsutil list-all-zones >/dev/null 2>&1 && break
    sleep 1
done
if pdnsutil list-zone shippin.tech.test >/dev/null 2>&1; then
    exit 0
fi
pdnsutil create-zone shippin.tech.test ns1.shippin.tech.test
pdnsutil set-kind shippin.tech.test native
pdnsutil replace-rrset shippin.tech.test '' SOA 'ns1.shippin.tech.test. admin.shippin.tech.test. 2024010101 3600 900 604800 86400'
pdnsutil add-record shippin.tech.test 'ns1' A '127.0.0.1'
pdnsutil add-record shippin.tech.test '' A '127.0.0.1'
pdnsutil add-record shippin.tech.test '' MX '10 mail.shippin.tech.test.'
pdnsutil add-record shippin.tech.test 'mail' A '127.0.0.1'
pdnsutil import-tsig-key shippin.tech-update-key hmac-sha256 'c3RhbHdhcnQtdGVzdC10c2lnLXNlY3JldC1rZXkxMjM0NTY3ODkw'
pdnsutil activate-tsig-key shippin.tech.test shippin.tech-update-key primary
pdnsutil set-meta shippin.tech.test TSIG-ALLOW-DNSUPDATE shippin.tech-update-key
pdnsutil set-meta shippin.tech.test ALLOW-DNSUPDATE-FROM '0.0.0.0/0'
"#;

pub async fn ensure_foundationdb() -> String {
    let container = FOUNDATIONDB
        .get_or_init(|| async {
            GenericImage::new("foundationdb/foundationdb", "7.4.6")
                .with_env_var("FDB_NETWORKING_MODE", "container")
                .with_mapped_port(4500, 4500.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-foundationdb")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start FoundationDB container")
        })
        .await;

    let start = Instant::now();
    loop {
        if fdbcli(container, "status minimal")
            .await
            .contains("The database is available")
        {
            break;
        }
        let created = fdbcli(container, "configure new single memory").await;
        if created.contains("Database created") || created.contains("Already exists") {
            continue;
        }
        if start.elapsed() > READY_TIMEOUT {
            panic!("Timed out configuring FoundationDB: {created}");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    let raw = exec_output(container, ["cat", "/var/fdb/fdb.cluster"]).await;
    let contents = raw.trim();
    if contents.is_empty() {
        panic!("FoundationDB cluster file was empty");
    }
    let path = std::env::temp_dir().join("shippin-tech-fdb.cluster");
    std::fs::write(&path, format!("{contents}\n"))
        .expect("Failed to write FoundationDB cluster file");
    path.to_string_lossy().into_owned()
}

async fn fdbcli(container: &ContainerAsync<GenericImage>, command: &str) -> String {
    exec_output(
        container,
        ["fdbcli", "--exec", command, "--timeout", "5"],
    )
    .await
}

async fn exec_output(
    container: &ContainerAsync<GenericImage>,
    command: impl IntoIterator<Item = impl Into<String>>,
) -> String {
    let mut result = container
        .exec(ExecCommand::new(command).with_cmd_ready_condition(CmdWaitFor::exit()))
        .await
        .expect("Failed to exec in container");
    let stdout = result.stdout_to_vec().await.unwrap_or_default();
    let stderr = result.stderr_to_vec().await.unwrap_or_default();
    format!(
        "{}{}",
        String::from_utf8_lossy(&stdout),
        String::from_utf8_lossy(&stderr)
    )
}

pub async fn ensure_postgres() {
    POSTGRES
        .get_or_init(|| async {
            GenericImage::new("postgres", "16-alpine")
                .with_wait_for(WaitFor::message_on_stderr(
                    "database system is ready to accept connections",
                ))
                .with_wait_for(WaitFor::message_on_stderr(
                    "database system is ready to accept connections",
                ))
                .with_env_var("POSTGRES_USER", "shippin.tech")
                .with_env_var("POSTGRES_PASSWORD", "shippin.tech")
                .with_env_var("POSTGRES_DB", "shippin.tech")
                .with_mapped_port(5432, 5432.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-postgres")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start PostgreSQL container")
        })
        .await;
    wait_for_tcp(5432).await;
}

pub async fn ensure_mysql() {
    MYSQL
        .get_or_init(|| async {
            GenericImage::new("mysql", "8.0")
                .with_wait_for(WaitFor::message_on_stderr("port: 3306  MySQL"))
                .with_env_var("MYSQL_ROOT_PASSWORD", "password")
                .with_env_var("MYSQL_DATABASE", "shippin.tech")
                .with_cmd(["--default-authentication-plugin=mysql_native_password"])
                .with_mapped_port(3307, 3306.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-mysql")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start MySQL container")
        })
        .await;
    wait_for_tcp(3307).await;
}

pub async fn ensure_mariadb() {
    MARIADB
        .get_or_init(|| async {
            GenericImage::new("mariadb", "11.4")
                .with_wait_for(WaitFor::message_on_stderr("port: 3306  mariadb.org"))
                .with_env_var("MARIADB_ROOT_PASSWORD", "password")
                .with_env_var("MARIADB_DATABASE", "shippin.tech")
                .with_mapped_port(3308, 3306.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-mariadb")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start MariaDB container")
        })
        .await;
    wait_for_tcp(3308).await;
}

pub async fn ensure_redis() {
    REDIS
        .get_or_init(|| async {
            GenericImage::new("redis", "7-alpine")
                .with_wait_for(WaitFor::message_on_stdout("Ready to accept connections"))
                .with_cmd(["redis-server", "--save", "", "--appendonly", "no"])
                .with_mapped_port(6379, 6379.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-redis")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start Redis container")
        })
        .await;
    wait_for_tcp(6379).await;
}

pub async fn ensure_nats() {
    NATS.get_or_init(|| async {
        GenericImage::new("nats", "latest")
            .with_wait_for(WaitFor::message_on_stderr("Server is ready"))
            .with_cmd(["--addr", "0.0.0.0", "--port", "4222", "--http_port", "8222"])
            .with_mapped_port(4222, 4222.tcp())
            .with_mapped_port(8222, 8222.tcp())
            .with_startup_timeout(READY_TIMEOUT)
            .with_container_name("shippin.tech-test-nats")
            .with_reuse(ReuseDirective::Always)
            .start()
            .await
            .expect("Failed to start NATS container")
    })
    .await;
    wait_for_tcp(4222).await;
}

pub async fn ensure_minio() {
    MINIO
        .get_or_init(|| async {
            GenericImage::new("cgr.dev/chainguard/minio", "latest")
                .with_env_var("MINIO_ROOT_USER", "minioadmin")
                .with_env_var("MINIO_ROOT_PASSWORD", "minioadmin")
                .with_cmd(["server", "/data", "--console-address", ":9001"])
                .with_mapped_port(9000, 9000.tcp())
                .with_mapped_port(9001, 9001.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-minio")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start MinIO container")
        })
        .await;
    wait_for_http("http://localhost:9000/minio/health/live").await;
    create_minio_bucket().await;
}

pub async fn ensure_opensearch() {
    OPENSEARCH
        .get_or_init(|| async {
            GenericImage::new("opensearchproject/opensearch", "2")
                .with_env_var("discovery.type", "single-node")
                .with_env_var("DISABLE_SECURITY_PLUGIN", "true")
                .with_env_var("OPENSEARCH_JAVA_OPTS", "-Xms1g -Xmx1g")
                .with_env_var("DISABLE_INSTALL_DEMO_CONFIG", "true")
                .with_mapped_port(9200, 9200.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-opensearch")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start OpenSearch container")
        })
        .await;
    wait_for_http("http://localhost:9200").await;
}

pub async fn ensure_meilisearch() {
    MEILISEARCH
        .get_or_init(|| async {
            GenericImage::new("getmeili/meilisearch", "latest")
                .with_env_var("MEILI_ENV", "development")
                .with_env_var("MEILI_NO_ANALYTICS", "true")
                .with_env_var("MEILI_MASTER_KEY", "shippin.tech-master-key")
                .with_mapped_port(7700, 7700.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-meilisearch")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start Meilisearch container")
        })
        .await;
    wait_for_http("http://localhost:7700/health").await;
}

pub async fn ensure_keycloak() {
    KEYCLOAK
        .get_or_init(|| async {
            GenericImage::new("quay.io/keycloak/keycloak", "latest")
                .with_env_var("KC_BOOTSTRAP_ADMIN_USERNAME", "admin")
                .with_env_var("KC_BOOTSTRAP_ADMIN_PASSWORD", "admin")
                .with_env_var("KC_HTTP_PORT", "9080")
                .with_env_var("KC_HEALTH_ENABLED", "true")
                .with_cmd(["start-dev", "--import-realm"])
                .with_copy_to(
                    "/opt/keycloak/data/import/shippin.tech-realm.json",
                    include_bytes!("../../docker/keycloak/shippin-tech-realm.json").to_vec(),
                )
                .with_mapped_port(9080, 9080.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-keycloak")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start Keycloak container")
        })
        .await;
    wait_for_http("http://localhost:9080/realms/shippin.tech/.well-known/openid-configuration").await;
}

pub async fn ensure_scim_tester() -> &'static ContainerAsync<GenericImage> {
    SCIM_TESTER
        .get_or_init(|| async {
            let image = GenericBuildableImage::new("shippin.tech-test-scim-tester", "local")
                .with_dockerfile_string(include_str!("../../docker/scim/Dockerfile"))
                .build_image()
                .await
                .expect("Failed to build the SCIM tester image");
            image
                .with_host("host.docker.internal", Host::HostGateway)
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-scim-tester")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start the SCIM tester container")
        })
        .await
}

pub async fn scim_tester_exec(args: &[&str]) -> (String, String) {
    let mut result = ensure_scim_tester()
        .await
        .exec(ExecCommand::new(args.iter().copied()).with_cmd_ready_condition(CmdWaitFor::exit()))
        .await
        .expect("Failed to exec the SCIM driver");
    let stdout = result.stdout_to_vec().await.unwrap_or_default();
    let stderr = result.stderr_to_vec().await.unwrap_or_default();

    (
        String::from_utf8_lossy(&stdout).into_owned(),
        String::from_utf8_lossy(&stderr).into_owned(),
    )
}

pub async fn ensure_acme() {
    ensure_challtestsrv().await;
    ensure_pebble().await;
}

async fn ensure_challtestsrv() {
    CHALLTESTSRV
        .get_or_init(|| async {
            let image = GenericBuildableImage::new("shippin.tech-test-challtestsrv", "local")
                .with_dockerfile_string(include_str!("../../docker/pebble/Dockerfile.challtestsrv"))
                .build_image()
                .await
                .expect("Failed to build challtestsrv image");
            image
                .with_network(ACME_NETWORK)
                .with_host("host.docker.internal", Host::HostGateway)
                .with_mapped_port(8055, 8055.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-challtestsrv")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start challtestsrv container")
        })
        .await;
    wait_for_tcp(8055).await;
}

async fn ensure_pebble() {
    PEBBLE
        .get_or_init(|| async {
            GenericImage::new("ghcr.io/letsencrypt/pebble", "latest")
                .with_env_var("PEBBLE_VA_NOSLEEP", "1")
                .with_env_var("PEBBLE_WFE_NONCEREJECT", "0")
                .with_env_var("PEBBLE_ALTERNATE_ROOTS", "2")
                .with_cmd([
                    "-config",
                    "/test/config/pebble-config.json",
                    "-dnsserver",
                    "shippin.tech-test-challtestsrv:8053",
                ])
                .with_copy_to(
                    "/test/config/pebble-config.json",
                    include_bytes!("../../docker/pebble/pebble-config.json").to_vec(),
                )
                .with_network(ACME_NETWORK)
                .with_host("host.docker.internal", Host::HostGateway)
                .with_mapped_port(14000, 14000.tcp())
                .with_mapped_port(15000, 15000.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-pebble")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start Pebble container")
        })
        .await;
    wait_for_tcp(14000).await;
}

pub async fn ensure_powerdns() {
    let container = POWERDNS
        .get_or_init(|| async {
            GenericImage::new("powerdns/pdns-auth-49", "latest")
                .with_wait_for(WaitFor::message_on_stderr("Creating backend connection"))
                .with_env_var("PDNS_AUTH_API_KEY", "shippin.tech-api-key")
                .with_copy_to(
                    "/etc/powerdns/pdns.d/shippin.tech.conf",
                    include_bytes!("../../docker/powerdns/pdns.conf").to_vec(),
                )
                .with_mapped_port(5300, 53.tcp())
                .with_mapped_port(5300, 53.udp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-powerdns")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start PowerDNS container")
        })
        .await;

    let mut result = container
        .exec(
            ExecCommand::new(["bash", "-c", POWERDNS_ZONE_INIT])
                .with_cmd_ready_condition(CmdWaitFor::exit()),
        )
        .await
        .expect("Failed to exec PowerDNS zone init");
    if result.exit_code().await.ok().flatten() != Some(0) {
        let stdout =
            String::from_utf8_lossy(&result.stdout_to_vec().await.unwrap_or_default()).into_owned();
        let stderr =
            String::from_utf8_lossy(&result.stderr_to_vec().await.unwrap_or_default()).into_owned();
        panic!("PowerDNS zone init failed:\n{stdout}\n{stderr}");
    }
    wait_for_tcp(5300).await;
}

pub async fn ensure_openldap() {
    const BOOTSTRAP_DIR: &str = "/container/service/slapd/assets/config/bootstrap/ldif/custom";
    let container = OPENLDAP
        .get_or_init(|| async {
            GenericImage::new("osixia/openldap", "1.5.0")
                .with_wait_for(WaitFor::message_on_stderr("slapd starting"))
                .with_env_var("LDAP_ORGANISATION", "shippin.tech Test")
                .with_env_var("LDAP_DOMAIN", "shippin.tech.test")
                .with_env_var("LDAP_BASE_DN", "dc=shippin.tech,dc=test")
                .with_env_var("LDAP_ADMIN_PASSWORD", "admin")
                .with_env_var("LDAP_TLS", "false")
                .with_copy_to(
                    format!("{BOOTSTRAP_DIR}/50-users.ldif"),
                    include_bytes!("../../docker/ldap/50-users.ldif").to_vec(),
                )
                .with_copy_to(
                    format!("{BOOTSTRAP_DIR}/60-groups.ldif"),
                    include_bytes!("../../docker/ldap/60-groups.ldif").to_vec(),
                )
                .with_mapped_port(389, 389.tcp())
                .with_startup_timeout(READY_TIMEOUT)
                .with_container_name("shippin.tech-test-openldap")
                .with_reuse(ReuseDirective::Always)
                .start()
                .await
                .expect("Failed to start OpenLDAP container")
        })
        .await;
    wait_for_tcp(389).await;

    let setup = format!("LDAPI={OPENLDAP_LDAPI_URL}\n{OPENLDAP_ALLOW_UNAUTHENTICATED_BIND}");
    let mut result = container
        .exec(
            ExecCommand::new(["bash", "-c", setup.as_str()])
                .with_cmd_ready_condition(CmdWaitFor::exit()),
        )
        .await
        .expect("Failed to exec OpenLDAP unauthenticated bind setup");
    if result.exit_code().await.ok().flatten() != Some(0) {
        let stdout =
            String::from_utf8_lossy(&result.stdout_to_vec().await.unwrap_or_default()).into_owned();
        let stderr =
            String::from_utf8_lossy(&result.stderr_to_vec().await.unwrap_or_default()).into_owned();
        panic!("OpenLDAP unauthenticated bind setup failed:\n{stdout}\n{stderr}");
    }
}

async fn create_minio_bucket() {
    use s3::{Bucket, BucketConfiguration, Region, creds::Credentials};

    let region = Region::Custom {
        region: "eu-central-1".to_string(),
        endpoint: "http://localhost:9000".to_string(),
    };
    let credentials = Credentials::new(Some("minioadmin"), Some("minioadmin"), None, None, None)
        .expect("Failed to build MinIO credentials");

    match Bucket::create_with_path_style(
        "shippin.tech",
        region,
        credentials,
        BucketConfiguration::default(),
    )
    .await
    {
        Ok(response) if response.success() => {}
        Ok(_) => {}
        Err(s3::error::S3Error::HttpFailWithBody(409, _)) => {}
        Err(err) => panic!("Failed to create MinIO bucket: {err:?}"),
    }
}

async fn wait_for_tcp(port: u16) {
    let start = Instant::now();
    loop {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }
        if start.elapsed() > READY_TIMEOUT {
            panic!("Timed out waiting for TCP port {port}");
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

async fn wait_for_http(url: &str) {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to build HTTP client");
    let start = Instant::now();
    loop {
        if let Ok(response) = client.get(url).send().await
            && response.status().is_success()
        {
            return;
        }
        if start.elapsed() > READY_TIMEOUT {
            panic!("Timed out waiting for {url}");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
