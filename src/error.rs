use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("HTTP error: {0}")]
    Request(String),

    #[error("There was an error sending the shutdown signal the polling task")]
    ShutdownPollingTaskFailed,

    #[error("There was an error joining the task")]
    TaskJoin(#[from] tokio::task::JoinError),

    #[error("C++ Exception caught: {0}")]
    CPPException(#[from] mediasoup_sys::Exception),

    #[error("Serde error: {0}")]
    SerdeJson(#[from] serde_json::Error),
}

impl From<surf::Error> for Error {
    fn from(error: surf::Error) -> Self {
        Error::Request(error.to_string())
    }
}
