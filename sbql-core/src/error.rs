//! Errors, in two shapes.
//!
//! [`SbqlError`] is the internal one: rich, `#[from]`-friendly, and what every
//! fallible function in this crate returns.
//!
//! [`CoreError`] is what leaves the crate, inside
//! [`CoreEvent::Error`](crate::CoreEvent::Error). Clients cannot match on
//! `SbqlError` — it is not `Clone`, it wraps driver types they have no business
//! knowing about, and over the FFI boundary it does not exist at all. So it is
//! flattened into something they *can* act on: a [`kind`](ErrorKind) to branch
//! on, a [`severity`](Severity), a one-line message for a status bar, and the
//! rest of the cause chain kept separately for when the user asks for it.
//!
//! This used to be a bare `String`, which meant the UI could only print it.
//! Deciding "should I suggest connecting?" or "is this worth a red bar?" was
//! left to substring matching on prose.

use thiserror::Error;

/// Everything that can go wrong inside the crate.
///
/// Rich and `#[from]`-friendly: it wraps driver errors whole so the cause chain
/// survives all the way to [`CoreError::detail`]. Clients get [`CoreError`]
/// instead — this type is not `Clone` and names types they have no business
/// knowing about.
///
/// `#[non_exhaustive]`: a new backend means a new variant, and that must not be
/// a breaking change. Match with a `_` arm.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SbqlError {
    /// Anything `sqlx` reported, for PostgreSQL, MySQL or SQLite. Covers both
    /// "the server rejected the statement" and "there was no server" — see
    /// [`ErrorKind::from`] for how those are told apart.
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// The connection file, or something asked of a connection that is not
    /// set up for it.
    #[error("Configuration error: {0}")]
    Config(String),

    /// The OS credential store could not be reached or refused the operation.
    /// Re-entering the password does not help; the store itself is the problem.
    #[error("Keyring error: {0}")]
    Keyring(String),

    /// The credential store is reachable but holds no password for this
    /// connection — unlike [`SbqlError::Keyring`], the user can fix this by
    /// re-entering the password.
    #[error("No saved password for '{0}'")]
    PasswordNotFound(String),

    /// sbql could not parse the SQL well enough to rewrite it, or the
    /// requested rewrite has no meaning on this backend.
    #[error("SQL parse error: {0}")]
    SqlParse(String),

    /// A pool could not be opened. The message names the connection, because
    /// this is what the user sees after pressing Enter on one.
    #[error("Connection error: {0}")]
    Connection(String),

    /// No saved or discovered connection has this id — it was deleted, or a
    /// stale id outlived a re-scan.
    #[error("Connection not found: {0}")]
    ConnectionNotFound(String),

    /// Nothing is connected. The distinct variant exists so a client can
    /// offer to connect instead of showing a failure.
    #[error("No active connection")]
    NoActiveConnection,

    /// Reading the catalog failed, or an edit was refused before it ran —
    /// an UPDATE with no primary key to match on, for instance.
    #[error("Schema introspection error: {0}")]
    Schema(String),

    /// Anything the redis client reported.
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    /// Anything the AWS SDK reported, already stringified — its error types
    /// are generic over the operation and do not fit one variant.
    #[error("DynamoDB error: {0}")]
    DynamoDb(String),

    /// Anything the MongoDB driver reported.
    #[error("MongoDB error: {0}")]
    MongoDb(String),

    /// Anything `tiberius` or its `bb8` pool reported.
    #[error("SQL Server error: {0}")]
    SqlServer(String),

    /// The tunnel could not be opened: the bastion refused, the key would not
    /// load, or its host key did not match `known_hosts`. The last of those is
    /// a refusal on purpose — see `tunnel::SshHandler`.
    #[error("SSH tunnel error: {0}")]
    SshTunnel(String),

    /// The file could not be read or did not have the shape the format
    /// requires. Note that an import is not transactional: this can arrive with
    /// rows already committed.
    #[error("Import error: {0}")]
    Import(String),

    /// Filesystem trouble — the config file, an import source, an export
    /// target.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML could not be produced or parsed. In practice: a hand-edited
    /// `connections.toml`.
    #[error("Serialization error: {0}")]
    Serialization(String),
}

impl From<russh::Error> for SbqlError {
    fn from(e: russh::Error) -> Self {
        Self::SshTunnel(e.to_string())
    }
}

/// What every fallible function in this crate returns.
pub type Result<T> = std::result::Result<T, SbqlError>;

// ---------------------------------------------------------------------------
// The client-facing error
// ---------------------------------------------------------------------------

/// What kind of thing went wrong, at the granularity a client can act on.
///
/// The point of each variant is that a UI does something *different* with it:
/// suggest opening a connection, put the cursor back in the editor, send the
/// user to the connection form. Variants that would all be handled the same way
/// are deliberately not split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// Nothing is connected yet. The user has to open a connection first.
    NoActiveConnection,
    /// A connection could not be opened, or was lost.
    Connection,
    /// The database rejected the statement, or there was nothing to run.
    Query,
    /// A saved connection or the config file itself is wrong.
    Config,
    /// The OS credential store could not be read or written.
    Credentials,
    /// Filesystem or import/export trouble.
    Io,
    /// Not classified. Treat as a plain failure.
    Other,
}

/// How bad it is.
///
/// Not every [`CoreError`] is a failure: saving a connection whose password the
/// keyring refuses still saves the connection. That used to be reported through
/// the same channel as a hard error and painted red, which told the user their
/// work had been lost when it had not.
/// `#[non_exhaustive]` for the same reason as [`ErrorKind`]: a third level
/// between "did not happen" and "happened partly" is plausible, and adding it
/// should not break a client's `match`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Severity {
    /// The operation did not happen.
    Error,
    /// The operation happened, but not completely.
    Warning,
}

/// A failure on its way out of the core.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreError {
    /// What went wrong, at the granularity a client branches on. This is the
    /// field to `match` — never the message.
    pub kind: ErrorKind,
    /// Whether the operation happened anyway. A [`Severity::Warning`] painted
    /// like a failure tells the user their work was lost when it was not.
    pub severity: Severity,
    /// One line, safe to put in a status bar.
    pub message: String,
    /// The causes underneath `message`, if it had any. Shown on request —
    /// this is where the driver's actual complaint usually is.
    pub detail: Option<String>,
}

impl CoreError {
    /// A failure with a message we wrote ourselves and no underlying cause.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            severity: Severity::Error,
            message: message.into(),
            detail: None,
        }
    }

    /// Something went wrong, but the operation still went through.
    pub fn warning(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            ..Self::new(kind, message)
        }
    }

    /// Attach an explanation the caller already has in hand.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Whether the operation went through despite this.
    pub fn is_warning(&self) -> bool {
        self.severity == Severity::Warning
    }
}

impl std::fmt::Display for CoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)?;
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CoreError {}

impl From<&SbqlError> for CoreError {
    fn from(e: &SbqlError) -> Self {
        Self {
            kind: ErrorKind::from(e),
            severity: Severity::Error,
            message: e.to_string(),
            detail: source_chain(e),
        }
    }
}

impl From<SbqlError> for CoreError {
    fn from(e: SbqlError) -> Self {
        Self::from(&e)
    }
}

impl From<&SbqlError> for ErrorKind {
    fn from(e: &SbqlError) -> Self {
        match e {
            SbqlError::NoActiveConnection => Self::NoActiveConnection,

            SbqlError::Connection(_)
            | SbqlError::ConnectionNotFound(_)
            | SbqlError::SshTunnel(_) => Self::Connection,

            // sqlx folds two very different situations into one type: the
            // server rejected our SQL, or we never reached a server at all.
            // Only the first is worth pointing the user back at the editor for.
            SbqlError::Database(inner) => match inner {
                sqlx::Error::PoolTimedOut
                | sqlx::Error::PoolClosed
                | sqlx::Error::Io(_)
                | sqlx::Error::Tls(_)
                | sqlx::Error::Configuration(_) => Self::Connection,
                _ => Self::Query,
            },

            SbqlError::SqlParse(_)
            | SbqlError::Schema(_)
            | SbqlError::Redis(_)
            | SbqlError::DynamoDb(_)
            | SbqlError::MongoDb(_)
            | SbqlError::SqlServer(_) => Self::Query,

            SbqlError::Config(_) | SbqlError::Serialization(_) => Self::Config,

            SbqlError::Keyring(_) | SbqlError::PasswordNotFound(_) => Self::Credentials,

            SbqlError::Io(_) | SbqlError::Import(_) => Self::Io,
        }
    }
}

/// Flatten everything below `err` into one line.
///
/// The outermost `Display` is frequently the least useful part of the chain —
/// `sqlx::Error` says "error returned from database" and keeps the server's
/// actual complaint in its source. Dropping it, as `e.to_string()` did, threw
/// away the only part worth reading.
///
/// Causes whose text already appears in the message above them are skipped:
/// `#[error("Database error: {0}")]` prints its source, so keeping both would
/// just say the same thing twice.
fn source_chain(err: &dyn std::error::Error) -> Option<String> {
    let top = err.to_string();
    let mut causes: Vec<String> = Vec::new();
    let mut current = err.source();

    while let Some(cause) = current {
        let text = cause.to_string();
        if !top.contains(&text) && !causes.iter().any(|seen| seen.contains(&text)) {
            causes.push(text);
        }
        current = cause.source();
    }

    (!causes.is_empty()).then(|| causes.join(": "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_connection_is_its_own_kind() {
        let err = CoreError::from(&SbqlError::NoActiveConnection);

        assert_eq!(err.kind, ErrorKind::NoActiveConnection);
        assert_eq!(err.severity, Severity::Error);
        assert_eq!(err.message, "No active connection");
    }

    /// The classification a client actually branches on: is the editor at
    /// fault, or the connection?
    #[test]
    fn an_unreachable_server_is_a_connection_problem_not_a_query_one() {
        let unreachable = CoreError::from(&SbqlError::Database(sqlx::Error::PoolTimedOut));
        assert_eq!(unreachable.kind, ErrorKind::Connection);

        let rejected = CoreError::from(&SbqlError::SqlParse("unexpected token".into()));
        assert_eq!(rejected.kind, ErrorKind::Query);
    }

    #[test]
    fn keyring_trouble_is_told_apart_from_config_trouble() {
        assert_eq!(
            ErrorKind::from(&SbqlError::PasswordNotFound("prod".into())),
            ErrorKind::Credentials
        );
        assert_eq!(
            ErrorKind::from(&SbqlError::Config("bad toml".into())),
            ErrorKind::Config
        );
    }

    /// The whole point of `detail`: the cause survives instead of being
    /// flattened away by `to_string()`.
    #[test]
    fn the_cause_underneath_is_kept() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "connections.toml");
        let err = CoreError::from(&SbqlError::Io(io));

        assert_eq!(err.kind, ErrorKind::Io);
        assert!(err.message.starts_with("IO error"), "{}", err.message);
    }

    /// `#[error("...: {0}")]` already prints its source, so the detail must not
    /// repeat it.
    #[test]
    fn a_cause_already_in_the_message_is_not_repeated() {
        let err = CoreError::from(&SbqlError::Redis(redis::RedisError::from((
            redis::ErrorKind::AuthenticationFailed,
            "wrong password",
        ))));

        assert!(err.message.contains("wrong password"), "{}", err.message);
        assert_eq!(err.detail, None, "the cause is already in the message");
    }

    #[test]
    fn a_warning_says_so() {
        let warn = CoreError::warning(ErrorKind::Credentials, "saved, but not the password");

        assert!(warn.is_warning());
        assert!(!CoreError::new(ErrorKind::Io, "nope").is_warning());
    }

    #[test]
    fn display_joins_the_message_and_its_detail() {
        let err = CoreError::new(ErrorKind::Query, "Query failed").with_detail("syntax error");
        assert_eq!(err.to_string(), "Query failed: syntax error");
    }
}
