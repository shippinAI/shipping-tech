

pub mod discovery;
pub mod integration;
pub mod issuer;
pub mod ldap;
pub mod oidc;
#[cfg(feature = "sqlite")]
pub mod sql;
pub mod synchronization;
pub mod unavailable;

#[tokio::test(flavor = "multi_thread")]
pub async fn directory_tests() {
    ldap::test().await;
    oidc::test().await;
    unavailable::test().await;
    discovery::test().await;
    issuer::test().await;
    #[cfg(feature = "sqlite")]
    sql::test().await;
    synchronization::test().await;
    integration::test().await;
}
