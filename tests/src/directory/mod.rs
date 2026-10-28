/*
 * SPDX-FileCopyrightText: 2020 shippin.tech Labs LLC <hello@support.shippin.tech>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

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
