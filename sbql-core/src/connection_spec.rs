//! Describes what a connection of each backend actually needs.
//!
//! Every client used to answer these questions on its own — which fields a
//! backend shows, what they are called, which are mandatory, how to turn typed
//! text into a [`ConnectionConfig`]. The TUI answered them in five parallel
//! `match` blocks and the macOS app barely answered them at all, so it happily
//! saved connections the TUI would reject.
//!
//! The answers live here once, as data. Adding a backend means adding one
//! [`BackendSpec`]; nothing else has to be kept in step.

use crate::config::{ConnectionConfig, SslMode};
use crate::pool::DbBackend;

// ---------------------------------------------------------------------------
// Fields
// ---------------------------------------------------------------------------

/// A value a user can supply for a connection.
///
/// These map onto [`ConnectionConfig`] storage, not onto labels: DynamoDB's
/// "Region" and MongoDB's "Database" are both [`ConnectionField::Database`],
/// because that is the field they are stored in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionField {
    /// What the connection is called in the list. Every backend asks for it.
    Name,
    /// Server hostname. DynamoDB labels it "Endpoint".
    Host,
    /// Server port.
    Port,
    /// Login user. DynamoDB labels it "Access Key".
    User,
    /// Database name. DynamoDB labels it "Region".
    Database,
    /// The secret. Never read back out of the credential store, so an edit
    /// form always shows it blank.
    Password,
    /// The SQLite file.
    FilePath,
    /// Chosen from a list rather than typed.
    SslMode,
}

impl ConnectionField {
    /// Whether the field is typed into. The others are cycled through choices,
    /// so a text cursor never lands on them.
    pub fn is_text(self) -> bool {
        !matches!(self, Self::SslMode)
    }

    /// Whether the value should be hidden while typing.
    pub fn is_secret(self) -> bool {
        matches!(self, Self::Password)
    }
}

/// One field as a particular backend presents it.
#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    /// Which value this is, i.e. where it is stored.
    pub field: ConnectionField,
    /// Backend-specific wording — DynamoDB calls `Host` "Endpoint".
    pub label: &'static str,
    /// Blank is rejected. The error names this field's `label`, not the
    /// storage field, so the user reads back the word they were shown.
    pub required: bool,
}

const fn field(field: ConnectionField, label: &'static str, required: bool) -> FieldSpec {
    FieldSpec {
        field,
        label,
        required,
    }
}

// ---------------------------------------------------------------------------
// Backends
// ---------------------------------------------------------------------------

/// Everything a client needs to present and validate one backend.
#[derive(Debug, Clone, Copy)]
pub struct BackendSpec {
    /// The backend this describes. Always matches the `DbBackend` it was
    /// looked up from.
    pub backend: DbBackend,
    /// Name shown in a backend picker.
    pub label: &'static str,
    /// Port pre-filled for a new connection.
    pub default_port: u16,
    /// Fields in the order they should be presented.
    pub fields: &'static [FieldSpec],
}

impl BackendSpec {
    /// The spec for a field, if this backend has it.
    pub fn field(&self, field: ConnectionField) -> Option<&'static FieldSpec> {
        self.fields.iter().find(|f| f.field == field)
    }

    pub(crate) fn has(&self, field: ConnectionField) -> bool {
        self.field(field).is_some()
    }
}

use ConnectionField as F;

const POSTGRES: BackendSpec = BackendSpec {
    backend: DbBackend::Postgres,
    label: "PostgreSQL",
    default_port: 5432,
    fields: &[
        field(F::Name, "Name", true),
        field(F::Host, "Host", true),
        field(F::Port, "Port", true),
        field(F::User, "User", true),
        field(F::Database, "Database", true),
        field(F::Password, "Password", false),
        field(F::SslMode, "SSL Mode", false),
    ],
};

const MYSQL: BackendSpec = BackendSpec {
    backend: DbBackend::Mysql,
    label: "MySQL",
    default_port: 3306,
    fields: &[
        field(F::Name, "Name", true),
        field(F::Host, "Host", true),
        field(F::Port, "Port", true),
        field(F::User, "User", true),
        field(F::Database, "Database", true),
        field(F::Password, "Password", false),
        field(F::SslMode, "SSL Mode", false),
    ],
};

const SQLITE: BackendSpec = BackendSpec {
    backend: DbBackend::Sqlite,
    label: "SQLite",
    default_port: 0,
    fields: &[
        field(F::Name, "Name", true),
        field(F::FilePath, "File Path", true),
    ],
};

const REDIS: BackendSpec = BackendSpec {
    backend: DbBackend::Redis,
    label: "Redis",
    default_port: 6379,
    fields: &[
        field(F::Name, "Name", true),
        field(F::Host, "Host", true),
        field(F::Port, "Port", true),
        field(F::Password, "Password", false),
        field(F::Database, "Database", false),
    ],
};

const DYNAMODB: BackendSpec = BackendSpec {
    backend: DbBackend::DynamoDb,
    label: "DynamoDB",
    default_port: 8000,
    fields: &[
        field(F::Name, "Name", true),
        field(F::Host, "Endpoint", true),
        field(F::Port, "Port", true),
        field(F::Database, "Region", true),
        field(F::User, "Access Key", false),
        field(F::Password, "Secret Key", false),
    ],
};

const MONGODB: BackendSpec = BackendSpec {
    backend: DbBackend::MongoDb,
    label: "MongoDB",
    default_port: 27017,
    fields: &[
        field(F::Name, "Name", true),
        field(F::Host, "Host", true),
        field(F::Port, "Port", true),
        field(F::Database, "Database", true),
        field(F::User, "User", false),
        field(F::Password, "Password", false),
    ],
};

const SQLSERVER: BackendSpec = BackendSpec {
    backend: DbBackend::SqlServer,
    label: "SQL Server",
    default_port: 1433,
    fields: &[
        field(F::Name, "Name", true),
        field(F::Host, "Host", true),
        field(F::Port, "Port", true),
        field(F::User, "User", false),
        field(F::Database, "Database", true),
        field(F::Password, "Password", false),
    ],
};

impl DbBackend {
    /// Every backend, in the order a picker should cycle through them.
    pub const ALL: [Self; 7] = [
        Self::Postgres,
        Self::Mysql,
        Self::Sqlite,
        Self::Redis,
        Self::DynamoDb,
        Self::MongoDb,
        Self::SqlServer,
    ];

    /// What this backend needs from the user.
    pub fn spec(self) -> &'static BackendSpec {
        match self {
            Self::Postgres => &POSTGRES,
            Self::Mysql => &MYSQL,
            Self::Sqlite => &SQLITE,
            Self::Redis => &REDIS,
            Self::DynamoDb => &DYNAMODB,
            Self::MongoDb => &MONGODB,
            Self::SqlServer => &SQLSERVER,
        }
    }

    /// Display name, e.g. "PostgreSQL".
    pub fn label(self) -> &'static str {
        self.spec().label
    }

    /// The next backend in [`DbBackend::ALL`], wrapping around.
    pub fn next(self) -> Self {
        let idx = Self::ALL.iter().position(|&b| b == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Why a draft could not become a [`ConnectionConfig`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    /// Which field to send the user back to.
    pub field: ConnectionField,
    /// Ready to show as-is, phrased in the backend's own wording for the
    /// field ("Region is required", not "Database is required").
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ValidationError {}

/// A connection as it is being typed: every value is still text, and any of it
/// may be wrong. [`ConnectionDraft::build`] is the only way to get a
/// [`ConnectionConfig`] out, so unvalidated input cannot reach storage.
#[derive(Clone, Default)]
pub struct ConnectionDraft {
    /// Which backend the draft is for. Decides which fields are shown and
    /// which are required.
    pub backend: DbBackend,
    /// Set when editing an existing connection; preserved through `build`.
    pub id: Option<uuid::Uuid>,
    /// What the connection will be called in the list.
    pub name: String,
    /// Server hostname. Empty for backends that have no host (SQLite).
    pub host: String,
    /// Port, still as text — `build` is where it has to parse as a `u16`.
    pub port: String,
    /// Login user. DynamoDB stores its access-key id here.
    pub user: String,
    /// Database name. DynamoDB stores its region here, MongoDB its database.
    pub database: String,
    /// Plaintext, for as long as the user is typing it. Never written to disk
    /// by this type: [`ConnectionDraft::password_for_save`] hands it to the
    /// save path, which puts it in the OS credential store.
    pub password: String,
    /// SQLite database file.
    pub file_path: String,
    /// TLS mode, for the backends that have one.
    pub ssl_mode: SslMode,
    /// SSH tunnel settings. The form does not expose these yet, but a draft
    /// carries them verbatim so editing a connection that *has* an SSH tunnel
    /// no longer silently strips it. `build` copies them straight back out.
    pub ssh_enabled: bool,
    /// Bastion hostname.
    pub ssh_host: String,
    /// Bastion port.
    pub ssh_port: u16,
    /// Bastion login user.
    pub ssh_user: String,
    /// `"password"` or `"key"`.
    pub ssh_auth_method: String,
    /// Private key file, when `ssh_auth_method` is `"key"`.
    pub ssh_key_path: Option<String>,
}

/// Written by hand rather than derived because `password` is plaintext for as
/// long as the user is typing it. A draft is exactly the kind of value that
/// ends up in a `tracing::debug!` of a UI event, and a derived `Debug` would
/// put the database password in the log file.
impl std::fmt::Debug for ConnectionDraft {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionDraft")
            .field("backend", &self.backend)
            .field("id", &self.id)
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("database", &self.database)
            .field("password", &Redacted(!self.password.is_empty()))
            .field("file_path", &self.file_path)
            .field("ssl_mode", &self.ssl_mode)
            .field("ssh_enabled", &self.ssh_enabled)
            .field("ssh_host", &self.ssh_host)
            .field("ssh_port", &self.ssh_port)
            .field("ssh_user", &self.ssh_user)
            .field("ssh_auth_method", &self.ssh_auth_method)
            .field("ssh_key_path", &self.ssh_key_path)
            .finish()
    }
}

/// Stands in for a secret in a `Debug` output: says whether one is set, never
/// what it is. Not even the length, which narrows a guess.
pub(crate) struct Redacted(pub(crate) bool);

impl std::fmt::Debug for Redacted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0 { "<redacted>" } else { "<unset>" })
    }
}

impl ConnectionDraft {
    /// A blank draft with this backend's sensible defaults.
    pub fn new(backend: DbBackend) -> Self {
        let spec = backend.spec();
        Self {
            backend,
            host: if spec.has(ConnectionField::Host) {
                "localhost".to_string()
            } else {
                String::new()
            },
            port: if spec.has(ConnectionField::Port) {
                spec.default_port.to_string()
            } else {
                String::new()
            },
            ..Default::default()
        }
    }

    /// Pre-fill from a saved connection. The password is never read back out of
    /// the credential store, so it always starts empty.
    pub fn from_config(cfg: &ConnectionConfig) -> Self {
        Self {
            backend: cfg.backend,
            id: Some(cfg.id),
            name: cfg.name.clone(),
            host: cfg.host.clone(),
            port: cfg.port.to_string(),
            user: cfg.user.clone(),
            database: cfg.database.clone(),
            password: String::new(),
            file_path: cfg.file_path.clone().unwrap_or_default(),
            ssl_mode: cfg.ssl_mode.clone(),
            ssh_enabled: cfg.ssh_enabled,
            ssh_host: cfg.ssh_host.clone(),
            ssh_port: cfg.ssh_port,
            ssh_user: cfg.ssh_user.clone(),
            ssh_auth_method: cfg.ssh_auth_method.clone(),
            ssh_key_path: cfg.ssh_key_path.clone(),
        }
    }

    /// Switch backend, re-defaulting the port if it still holds the old
    /// backend's default (so a deliberately typed port survives).
    pub fn set_backend(&mut self, backend: DbBackend) {
        let previous_default = self.backend.spec().default_port.to_string();
        if self.port.is_empty() || self.port == previous_default {
            let spec = backend.spec();
            self.port = if spec.has(ConnectionField::Port) {
                spec.default_port.to_string()
            } else {
                String::new()
            };
        }
        if self.host.is_empty() && backend.spec().has(ConnectionField::Host) {
            self.host = "localhost".to_string();
        }
        self.backend = backend;
    }

    /// Read a field by name, so a form can iterate
    /// [`BackendSpec::fields`] instead of hard-coding which member goes in
    /// which box. `SslMode` reads back as its wire spelling.
    pub fn value(&self, field: ConnectionField) -> &str {
        match field {
            ConnectionField::Name => &self.name,
            ConnectionField::Host => &self.host,
            ConnectionField::Port => &self.port,
            ConnectionField::User => &self.user,
            ConnectionField::Database => &self.database,
            ConnectionField::Password => &self.password,
            ConnectionField::FilePath => &self.file_path,
            ConnectionField::SslMode => self.ssl_mode.as_str(),
        }
    }

    /// Mutable access for text fields. Returns `None` for fields that are
    /// cycled rather than typed.
    pub fn value_mut(&mut self, field: ConnectionField) -> Option<&mut String> {
        match field {
            ConnectionField::Name => Some(&mut self.name),
            ConnectionField::Host => Some(&mut self.host),
            ConnectionField::Port => Some(&mut self.port),
            ConnectionField::User => Some(&mut self.user),
            ConnectionField::Database => Some(&mut self.database),
            ConnectionField::Password => Some(&mut self.password),
            ConnectionField::FilePath => Some(&mut self.file_path),
            ConnectionField::SslMode => None,
        }
    }

    /// What this draft's backend asks for — the field list to render.
    pub fn spec(&self) -> &'static BackendSpec {
        self.backend.spec()
    }

    /// Validate and convert. This is the single gate every client goes through.
    ///
    /// # Errors
    ///
    /// A required field is blank, or the port is not a number in 1-65535. The
    /// [`ValidationError`] names the offending [`ConnectionField`], so the
    /// caller's job is to put the cursor back in that box and show the message
    /// — not to guess from the text which field went wrong.
    pub fn build(&self) -> std::result::Result<ConnectionConfig, ValidationError> {
        let spec = self.spec();

        for f in spec.fields {
            if f.required && self.value(f.field).trim().is_empty() {
                return Err(ValidationError {
                    field: f.field,
                    message: format!("{} is required", f.label),
                });
            }
        }

        let port = if spec.has(ConnectionField::Port) {
            self.port
                .trim()
                .parse::<u16>()
                .map_err(|_| ValidationError {
                    field: ConnectionField::Port,
                    message: "Port must be a number (1-65535)".to_string(),
                })?
        } else {
            0
        };

        let name = self.name.trim();
        let host = self.host.trim();
        let user = self.user.trim();
        let database = self.database.trim();

        let mut config = match self.backend {
            DbBackend::Postgres => ConnectionConfig::new_postgres(name, host, port, user, database),
            DbBackend::Mysql => ConnectionConfig::new_mysql(name, host, port, user, database),
            DbBackend::Sqlite => ConnectionConfig::new_sqlite(name, self.file_path.trim()),
            DbBackend::Redis => {
                let mut c = ConnectionConfig::new_redis(name, host, port);
                // Redis keeps its "0" default when the user leaves it blank.
                if !database.is_empty() {
                    c.database = database.to_string();
                }
                c
            }
            DbBackend::DynamoDb => {
                let mut c = ConnectionConfig::new_dynamodb(name, host, port, database);
                c.user = user.to_string();
                c
            }
            DbBackend::MongoDb => {
                let mut c = ConnectionConfig::new_mongodb(name, host, port, database);
                c.user = user.to_string();
                c
            }
            DbBackend::SqlServer => {
                ConnectionConfig::new_sqlserver(name, host, port, user, database)
            }
        };

        if spec.has(ConnectionField::SslMode) {
            config.ssl_mode = self.ssl_mode.clone();
        }
        // Carry SSH settings straight back out. The form cannot edit them, so
        // for an SSH-tunneled connection this is the difference between an edit
        // preserving the tunnel and silently deleting it.
        if self.ssh_enabled {
            config.ssh_enabled = true;
            config.ssh_host.clone_from(&self.ssh_host);
            config.ssh_port = self.ssh_port;
            config.ssh_user.clone_from(&self.ssh_user);
            config.ssh_auth_method.clone_from(&self.ssh_auth_method);
            config.ssh_key_path.clone_from(&self.ssh_key_path);
        }
        if let Some(id) = self.id {
            config.id = id;
        }

        Ok(config)
    }

    /// The password to hand to `SaveConnection`.
    ///
    /// `None` means "keep whatever is already stored" — only meaningful when
    /// editing, where a blank box means the user did not want to change it.
    pub fn password_for_save(&self) -> Option<String> {
        if self.password.is_empty() && self.id.is_some() {
            None
        } else {
            Some(self.password.clone())
        }
    }
}

impl ConnectionConfig {
    /// Check an already-typed config against its backend's requirements.
    ///
    /// [`ConnectionDraft::build`] covers clients that collect text, but a
    /// client can also hand over a fully-typed config — the macOS app does,
    /// through UniFFI. Both routes end at the same rules, and the save handler
    /// calls this so nothing reaches disk unvalidated.
    ///
    /// # Errors
    ///
    /// A field this backend requires is empty (or the port is 0). As with
    /// [`ConnectionDraft::build`], the error names the field — send the user
    /// back to it.
    pub fn validate(&self) -> std::result::Result<(), ValidationError> {
        let spec = self.backend.spec();

        for f in spec.fields {
            if !f.required {
                continue;
            }
            let missing = match f.field {
                ConnectionField::Name => self.name.trim().is_empty(),
                ConnectionField::Host => self.host.trim().is_empty(),
                ConnectionField::Port => self.port == 0,
                ConnectionField::User => self.user.trim().is_empty(),
                ConnectionField::Database => self.database.trim().is_empty(),
                ConnectionField::FilePath => {
                    self.file_path.as_deref().unwrap_or("").trim().is_empty()
                }
                // Never required, and not stored as text.
                ConnectionField::Password | ConnectionField::SslMode => false,
            };
            if missing {
                return Err(ValidationError {
                    field: f.field,
                    message: format!("{} is required", f.label),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `Debug` impl is hand-written for exactly this reason, and a derive
    /// would silently undo it. A draft holds the password in plaintext while
    /// the user types, and drafts travel inside UI events that get logged.
    #[test]
    fn a_drafts_password_never_reaches_its_debug_output() {
        let mut draft = ConnectionDraft::new(DbBackend::Postgres);
        draft.name = "prod".into();
        draft.password = "hunter2-not-a-real-password".into();

        let shown = format!("{draft:?}");

        assert!(
            !shown.contains("hunter2"),
            "the password leaked into Debug: {shown}"
        );
        assert!(shown.contains("<redacted>"), "{shown}");
        // The rest of the draft still has to be debuggable.
        assert!(shown.contains("prod"), "{shown}");
    }

    /// "Set" and "not set" are worth telling apart when debugging a failed
    /// save; the value itself never is.
    #[test]
    fn a_blank_password_debugs_as_unset_rather_than_redacted() {
        let draft = ConnectionDraft::new(DbBackend::Postgres);
        assert!(format!("{draft:?}").contains("<unset>"));
    }

    #[test]
    fn every_backend_has_a_spec_that_matches_its_tag() {
        for backend in DbBackend::ALL {
            let spec = backend.spec();
            assert_eq!(spec.backend, backend, "spec/backend mismatch");
            assert!(!spec.label.is_empty());
            assert!(
                spec.has(ConnectionField::Name),
                "{backend:?} must ask for a name"
            );
        }
    }

    #[test]
    fn cycling_visits_every_backend_and_returns_to_the_start() {
        let mut seen = Vec::new();
        let mut b = DbBackend::Postgres;
        for _ in 0..DbBackend::ALL.len() {
            seen.push(b);
            b = b.next();
        }
        assert_eq!(b, DbBackend::Postgres, "cycle must wrap around");
        assert_eq!(seen.len(), DbBackend::ALL.len());
        for backend in DbBackend::ALL {
            assert!(
                seen.contains(&backend),
                "{backend:?} unreachable by cycling"
            );
        }
    }

    #[test]
    fn a_field_is_never_listed_twice() {
        for backend in DbBackend::ALL {
            let fields: Vec<_> = backend.spec().fields.iter().map(|f| f.field).collect();
            let mut unique = fields.clone();
            unique.sort_by_key(|f| format!("{f:?}"));
            unique.dedup();
            assert_eq!(unique.len(), fields.len(), "{backend:?} repeats a field");
        }
    }

    #[test]
    fn missing_required_field_names_itself() {
        let mut draft = ConnectionDraft::new(DbBackend::Postgres);
        draft.name = "local".into();
        draft.user = "postgres".into();
        // database left blank

        let err = draft.build().expect_err("should reject a blank database");
        assert_eq!(err.field, ConnectionField::Database);
        assert_eq!(err.message, "Database is required");
    }

    #[test]
    fn labels_come_from_the_backend_not_the_storage_field() {
        let mut draft = ConnectionDraft::new(DbBackend::DynamoDb);
        draft.name = "aws".into();
        // Region is stored in `database`, but must be reported by its own name.
        let err = draft.build().expect_err("should reject a blank region");
        assert_eq!(err.field, ConnectionField::Database);
        assert_eq!(err.message, "Region is required");
    }

    #[test]
    fn a_bad_port_is_rejected() {
        let mut draft = ConnectionDraft::new(DbBackend::Postgres);
        draft.name = "local".into();
        draft.user = "postgres".into();
        draft.database = "postgres".into();
        draft.port = "not-a-port".into();

        let err = draft.build().expect_err("should reject a non-numeric port");
        assert_eq!(err.field, ConnectionField::Port);
        assert_eq!(err.message, "Port must be a number (1-65535)");

        draft.port = "70000".into(); // beyond u16
        assert!(draft.build().is_err(), "should reject an out-of-range port");
    }

    #[test]
    fn sqlite_needs_only_a_name_and_a_path() {
        let mut draft = ConnectionDraft::new(DbBackend::Sqlite);
        draft.name = "local".into();
        assert!(draft.build().is_err(), "file path is required");

        draft.file_path = "/tmp/demo.db".into();
        let cfg = draft.build().expect("valid sqlite draft");
        assert_eq!(cfg.backend, DbBackend::Sqlite);
        assert_eq!(cfg.file_path.as_deref(), Some("/tmp/demo.db"));
    }

    #[test]
    fn defaults_follow_the_backend() {
        assert_eq!(ConnectionDraft::new(DbBackend::Postgres).port, "5432");
        assert_eq!(ConnectionDraft::new(DbBackend::Mysql).port, "3306");
        assert_eq!(ConnectionDraft::new(DbBackend::MongoDb).port, "27017");
        assert_eq!(ConnectionDraft::new(DbBackend::Sqlite).port, "");
        assert_eq!(ConnectionDraft::new(DbBackend::Sqlite).host, "");
    }

    #[test]
    fn switching_backend_updates_an_untouched_port_but_keeps_a_typed_one() {
        let mut draft = ConnectionDraft::new(DbBackend::Postgres);
        draft.set_backend(DbBackend::Mysql);
        assert_eq!(draft.port, "3306", "untouched default should follow");

        draft.port = "6543".into();
        draft.set_backend(DbBackend::MongoDb);
        assert_eq!(draft.port, "6543", "a typed port must survive");
    }

    #[test]
    fn editing_keeps_the_id_and_a_blank_password_means_unchanged() {
        let original = ConnectionConfig::new_postgres("db", "localhost", 5432, "u", "d");
        let draft = ConnectionDraft::from_config(&original);

        assert!(draft.password.is_empty(), "password is never read back");
        assert_eq!(draft.build().expect("valid").id, original.id);
        assert_eq!(draft.password_for_save(), None, "blank means keep existing");
    }

    #[test]
    fn a_new_connection_with_a_blank_password_still_stores_an_empty_one() {
        let mut draft = ConnectionDraft::new(DbBackend::Redis);
        draft.name = "cache".into();
        assert_eq!(draft.password_for_save(), Some(String::new()));
    }

    /// Editing a connection that has an SSH tunnel must not erase it — the
    /// form does not show SSH fields, so a draft round-trip has to preserve
    /// them.
    #[test]
    fn editing_preserves_ssh_tunnel_settings() {
        let mut original = ConnectionConfig::new_postgres("db", "localhost", 5432, "u", "d");
        original.ssh_enabled = true;
        original.ssh_host = "bastion.example.com".into();
        original.ssh_port = 2222;
        original.ssh_user = "jump".into();
        original.ssh_auth_method = "key".into();
        original.ssh_key_path = Some("/home/me/.ssh/id_ed25519".into());

        let rebuilt = ConnectionDraft::from_config(&original)
            .build()
            .expect("valid");

        assert!(rebuilt.ssh_enabled);
        assert_eq!(rebuilt.ssh_host, "bastion.example.com");
        assert_eq!(rebuilt.ssh_port, 2222);
        assert_eq!(rebuilt.ssh_user, "jump");
        assert_eq!(rebuilt.ssh_auth_method, "key");
        assert_eq!(
            rebuilt.ssh_key_path.as_deref(),
            Some("/home/me/.ssh/id_ed25519")
        );
    }

    #[test]
    fn redis_keeps_its_default_database_when_left_blank() {
        let mut draft = ConnectionDraft::new(DbBackend::Redis);
        draft.name = "cache".into();
        assert_eq!(draft.build().expect("valid").database, "0");

        draft.database = "3".into();
        assert_eq!(draft.build().expect("valid").database, "3");
    }

    #[test]
    fn ssl_mode_is_only_applied_where_the_backend_has_it() {
        let mut draft = ConnectionDraft::new(DbBackend::Postgres);
        draft.name = "db".into();
        draft.user = "u".into();
        draft.database = "d".into();
        draft.ssl_mode = SslMode::Require;
        assert_eq!(draft.build().expect("valid").ssl_mode, SslMode::Require);

        let mut redis = ConnectionDraft::new(DbBackend::Redis);
        redis.name = "cache".into();
        redis.ssl_mode = SslMode::Require;
        assert_ne!(redis.build().expect("valid").ssl_mode, SslMode::Require);
    }
}
