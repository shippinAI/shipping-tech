use std::time::Duration;

use store::write::now;

#[derive(Clone)]
pub struct LicenseKey {
    pub valid_to: u64,
    pub valid_from: u64,
    pub domain: String,
    pub accounts: u32,
}

pub struct RenewedLicense {
    pub encoded_key: String,
    pub key: LicenseKey,
}

impl LicenseKey {
    pub fn new(_license_key: impl Into<String>, _server_hostname: &str) -> Result<Self, &'static str> {
        Err("enterprise license validator is not in this tree")
    }

    pub fn invalid(server_hostname: &str) -> Self {
        Self {
            valid_to: 0,
            valid_from: 0,
            domain: server_hostname.to_string(),
            accounts: 0,
        }
    }

    pub fn is_expired(&self) -> bool {
        now() >= self.valid_to
    }

    pub fn is_near_expiration(&self) -> bool {
        self.expires_in() < Duration::from_secs(86400 * 30)
    }

    pub fn expires_in(&self) -> Duration {
        Duration::from_secs(self.valid_to.saturating_sub(now()))
    }

    pub fn renew_in(&self) -> Duration {
        let left = self.expires_in();
        if left > Duration::from_secs(86400) {
            left - Duration::from_secs(86400)
        } else {
            Duration::from_secs(3600)
        }
    }

    pub async fn try_renew(self, _api_key: &str) -> Result<RenewedLicense, &'static str> {
        Err("enterprise license renewal is not in this tree")
    }
}
