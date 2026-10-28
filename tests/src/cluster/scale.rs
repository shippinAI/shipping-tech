use crate::{
    imap::append::assert_append_message,
    utils::{
        imap::{ImapConnection, Type},
        server::TestServerBuilder,
    },
};
use imap_proto::ResponseType;
use registry::{
    schema::{
        enums::NetworkListenerProtocol,
        prelude::{ObjectType, SocketAddr},
        structs::{
            BlobStore, ClusterListenerGroup, ClusterListenerGroupProperties, ClusterRole,
            ClusterTaskGroup, Coordinator, Imap, NatsCoordinator, NetworkListener,
            PublicStringOptional, PublicStringValue, S3Store, S3StoreCustomRegion, S3StoreRegion,
            SecretKeyOptional, SecretKeyValue,
        },
    },
    types::map::Map,
};
use std::str::FromStr;
use store::registry::RegistryQuery;
use types::id::Id;

const BODY: &str = "scale-body-from-node-a";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "STORE=FoundationDb cargo test -p tests --features scale -- --ignored scale_two_nodes"]
pub async fn scale_two_nodes() {
    if std::env::var("STORE").ok().as_deref() != Some("FoundationDb") {
        panic!(
            "Set STORE=FoundationDb so both nodes share one cluster file. \
             Example: STORE=FoundationDb cargo test -p tests --features scale -- --ignored scale_two_nodes"
        );
    }

    crate::utils::containers::ensure_minio().await;
    crate::utils::containers::ensure_nats().await;

    let mut servers = Vec::with_capacity(2);
    let test = TestServerBuilder::new("scale_node_a")
        .await
        .with_object(Coordinator::Nats(NatsCoordinator {
            addresses: Map::new(vec!["127.0.0.1:4222".to_string()]),
            use_tls: false,
            ..Default::default()
        }))
        .await
        .with_object(BlobStore::S3(S3Store {
            access_key: PublicStringOptional::Value(PublicStringValue {
                value: "minioadmin".into(),
            }),
            bucket: "shippin.tech".into(),
            region: S3StoreRegion::Custom(S3StoreCustomRegion {
                custom_endpoint: "http://localhost:9000".into(),
                custom_region: "eu-central-1".into(),
            }),
            secret_key: SecretKeyOptional::Value(SecretKeyValue {
                secret: "minioadmin".into(),
            }),
            allow_invalid_certs: true,
            ..Default::default()
        }))
        .await
        .with_object(Imap {
            allow_plain_text_auth: true,
            ..Default::default()
        })
        .await
        .with_listener(NetworkListenerProtocol::Http, "http_0", 11000, true)
        .await
        .with_listener(NetworkListenerProtocol::Imap, "imap_0", 12000, false)
        .await
        .build()
        .await;

    let admin = test.account("admin");
    admin
        .create_user_account(
            "jdoe@example.com",
            "this is john's secret",
            "John's account",
            &[],
            vec![],
        )
        .await;
    admin.reload_settings().await;

    let mut listeners = vec![
        test.server
            .registry()
            .query::<Vec<Id>>(RegistryQuery::new(ObjectType::NetworkListener))
            .await
            .unwrap(),
    ];
    let http_listener_id = admin
        .registry_create_object(NetworkListener {
            name: "http_1".to_string(),
            bind: Map::new(vec![SocketAddr::from_str("127.0.0.1:11001").unwrap()]),
            protocol: NetworkListenerProtocol::Http,
            tls_implicit: true,
            use_tls: true,
            ..Default::default()
        })
        .await;
    let imap_listener_id = admin
        .registry_create_object(NetworkListener {
            name: "imap_1".to_string(),
            bind: Map::new(vec![SocketAddr::from_str("127.0.0.1:12001").unwrap()]),
            protocol: NetworkListenerProtocol::Imap,
            tls_implicit: false,
            use_tls: true,
            ..Default::default()
        })
        .await;
    listeners.push(vec![http_listener_id, imap_listener_id]);

    for (role_id, listener_ids) in listeners.into_iter().enumerate() {
        admin
            .registry_create_object(ClusterRole {
                name: format!("role_{role_id}"),
                listeners: ClusterListenerGroup::EnableSome(ClusterListenerGroupProperties {
                    listener_ids: Map::new(listener_ids),
                }),
                tasks: ClusterTaskGroup::EnableAll,
                description: None,
            })
            .await;
    }
    let node_a = test.server.registry().node_id();
    servers.push(test);

    let node_b = TestServerBuilder::new_with_role(
        "scale_node_b",
        "mail-b.example.com".to_string(),
        Some("role_1".to_string()),
        false,
    )
    .await
    .build_with_opts(false)
    .await;
    let node_b_id = node_b.server.registry().node_id();
    assert_ne!(
        node_a, node_b_id,
        "nodes that share FoundationDB must lease different ids"
    );
    servers.push(node_b);

    let message = format!("Subject: scale-node-a\r\n\r\n{BODY}\r\n");
    let mut writer = imap_client(0).await;
    let uid = assert_append_message(&mut writer, "INBOX", &message, ResponseType::Ok)
        .await
        .into_append_uid();
    servers[0].wait_for_tasks().await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mut reader = imap_client(1).await;
    reader.send("SELECT INBOX").await;
    reader.assert_read(Type::Tagged, ResponseType::Ok).await;
    reader
        .send(&format!("UID FETCH {uid} (BODY.PEEK[TEXT])"))
        .await;
    reader
        .assert_read(Type::Tagged, ResponseType::Ok)
        .await
        .assert_contains(BODY);
}

async fn imap_client(node_id: u32) -> ImapConnection {
    let mut conn = ImapConnection::connect_to(b"A1 ", format!("127.0.0.1:1200{node_id}")).await;
    conn.assert_read(Type::Untagged, ResponseType::Ok).await;
    conn.authenticate("jdoe@example.com", "this is john's secret")
        .await;
    conn
}
