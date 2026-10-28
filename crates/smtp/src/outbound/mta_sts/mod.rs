

pub mod lookup;
pub mod parse;
pub mod verify;

#[derive(Debug)]
pub enum Error {
    Dns(mail_auth::Error),
    Http(reqwest::Error),
    InvalidPolicy(String),
}
