//! Error conversion between [`ClientError`] and sqlx errors.

use crate::client::ClientError;

impl From<ClientError> for sqlx_core::Error {
    fn from(err: ClientError) -> Self {
        match err {
            ClientError::Request(e) => sqlx_core::Error::Io(std::io::Error::other(e.to_string())),
            ClientError::QueryFailed { status, body } => {
                sqlx_core::Error::Protocol(format!("HTTP {status}: {body}"))
            }
            ClientError::Deserialize(e) => sqlx_core::Error::Protocol(e.to_string()),
            ClientError::RowDeserialize(e) => sqlx_core::Error::Decode(e.into()),
            ClientError::ClientBuild(e) => sqlx_core::Error::Configuration(e.into()),
        }
    }
}
