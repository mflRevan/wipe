//! Error type shared across `wipe-core`.

use std::path::Path;

/// Convenient result alias for the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// All errors that `wipe-core` can produce.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// No `.wipe` board was found at the given path or any of its ancestors.
    #[error("no wipe board found in `{0}` or any parent directory; run `wipe init` first")]
    NotInitialized(String),

    /// A `.wipe` board already exists where one was about to be created.
    #[error("a wipe board already exists at `{0}`")]
    AlreadyInitialized(String),

    /// A ticket with the given ID does not exist.
    #[error("ticket `{0}` not found")]
    TicketNotFound(String),

    /// A list with the given ID does not exist.
    #[error("list `{0}` not found")]
    ListNotFound(String),

    /// A forum thread with the given ID does not exist.
    #[error("forum thread `{0}` not found")]
    ThreadNotFound(String),

    /// A forum post with the given ID does not exist.
    #[error("forum post `{0}` not found")]
    PostNotFound(String),

    /// A generic, contextual error message.
    #[error("{0}")]
    Message(String),

    /// An underlying filesystem error.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// A JSON (de)serialization error.
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl Error {
    /// Build a [`Error::Message`] from anything string-like.
    pub fn msg(m: impl Into<String>) -> Self {
        Error::Message(m.into())
    }

    /// Build a [`Error::NotInitialized`] from a path.
    pub fn not_initialized(p: impl AsRef<Path>) -> Self {
        Error::NotInitialized(p.as_ref().display().to_string())
    }
}
