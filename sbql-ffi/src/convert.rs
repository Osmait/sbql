//! Conversions between `sbql-core` types and FFI-safe UniFFI types.

use crate::*;

// ---------------------------------------------------------------------------
// ConnectionConfig ↔ FfiConnectionConfig
// ---------------------------------------------------------------------------

impl From<sbql_core::ConnectionConfig> for FfiConnectionConfig {
    fn from(c: sbql_core::ConnectionConfig) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name,
            backend: c.backend.into(),
            host: c.host,
            port: c.port,
            user: c.user,
            database: c.database,
            ssl_mode: c.ssl_mode.into(),
            file_path: c.file_path,
        }
    }
}

impl TryFrom<FfiConnectionConfig> for sbql_core::ConnectionConfig {
    type Error = SbqlFfiError;

    fn try_from(c: FfiConnectionConfig) -> Result<Self, SbqlFfiError> {
        let id = uuid::Uuid::parse_str(&c.id)
            .map_err(|e| SbqlFfiError::InvalidArgument { msg: format!("Invalid UUID: {e}") })?;
        Ok(sbql_core::ConnectionConfig {
            id,
            name: c.name,
            backend: c.backend.into(),
            host: c.host,
            port: c.port,
            user: c.user,
            database: c.database,
            ssl_mode: c.ssl_mode.into(),
            file_path: c.file_path,
        })
    }
}

// ---------------------------------------------------------------------------
// DbBackend ↔ FfiDbBackend
// ---------------------------------------------------------------------------

impl From<sbql_core::DbBackend> for FfiDbBackend {
    fn from(b: sbql_core::DbBackend) -> Self {
        match b {
            sbql_core::DbBackend::Postgres => FfiDbBackend::Postgres,
            sbql_core::DbBackend::Sqlite => FfiDbBackend::Sqlite,
        }
    }
}

impl From<FfiDbBackend> for sbql_core::DbBackend {
    fn from(b: FfiDbBackend) -> Self {
        match b {
            FfiDbBackend::Postgres => sbql_core::DbBackend::Postgres,
            FfiDbBackend::Sqlite => sbql_core::DbBackend::Sqlite,
        }
    }
}

// ---------------------------------------------------------------------------
// SslMode ↔ FfiSslMode
// ---------------------------------------------------------------------------

impl From<sbql_core::SslMode> for FfiSslMode {
    fn from(s: sbql_core::SslMode) -> Self {
        match s {
            sbql_core::SslMode::Prefer => FfiSslMode::Prefer,
            sbql_core::SslMode::Disable => FfiSslMode::Disable,
            sbql_core::SslMode::Require => FfiSslMode::Require,
            sbql_core::SslMode::VerifyCa => FfiSslMode::VerifyCa,
            sbql_core::SslMode::VerifyFull => FfiSslMode::VerifyFull,
        }
    }
}

impl From<FfiSslMode> for sbql_core::SslMode {
    fn from(s: FfiSslMode) -> Self {
        match s {
            FfiSslMode::Prefer => sbql_core::SslMode::Prefer,
            FfiSslMode::Disable => sbql_core::SslMode::Disable,
            FfiSslMode::Require => sbql_core::SslMode::Require,
            FfiSslMode::VerifyCa => sbql_core::SslMode::VerifyCa,
            FfiSslMode::VerifyFull => sbql_core::SslMode::VerifyFull,
        }
    }
}

// ---------------------------------------------------------------------------
// SortDirection ↔ FfiSortDirection
// ---------------------------------------------------------------------------

impl From<FfiSortDirection> for sbql_core::SortDirection {
    fn from(d: FfiSortDirection) -> Self {
        match d {
            FfiSortDirection::Ascending => sbql_core::SortDirection::Ascending,
            FfiSortDirection::Descending => sbql_core::SortDirection::Descending,
        }
    }
}

// ---------------------------------------------------------------------------
// QueryResult → FfiQueryResult
// ---------------------------------------------------------------------------

impl From<sbql_core::QueryResult> for FfiQueryResult {
    fn from(r: sbql_core::QueryResult) -> Self {
        Self {
            columns: r.columns,
            rows: r.rows,
            page: r.page as u32,
            has_next_page: r.has_next_page,
        }
    }
}

// ---------------------------------------------------------------------------
// TableEntry → FfiTableEntry
// ---------------------------------------------------------------------------

impl From<sbql_core::TableEntry> for FfiTableEntry {
    fn from(t: sbql_core::TableEntry) -> Self {
        Self {
            schema: t.schema,
            name: t.name,
        }
    }
}

// ---------------------------------------------------------------------------
// ColumnInfo → FfiColumnInfo
// ---------------------------------------------------------------------------

impl From<sbql_core::ColumnInfo> for FfiColumnInfo {
    fn from(c: sbql_core::ColumnInfo) -> Self {
        Self {
            name: c.name,
            data_type: c.data_type,
            is_pk: c.is_pk,
            is_nullable: c.is_nullable,
        }
    }
}

// ---------------------------------------------------------------------------
// TableSchema → FfiTableSchema
// ---------------------------------------------------------------------------

impl From<sbql_core::TableSchema> for FfiTableSchema {
    fn from(t: sbql_core::TableSchema) -> Self {
        Self {
            schema: t.schema,
            name: t.name,
            columns: t.columns.into_iter().map(Into::into).collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// ForeignKey → FfiForeignKey
// ---------------------------------------------------------------------------

impl From<sbql_core::ForeignKey> for FfiForeignKey {
    fn from(f: sbql_core::ForeignKey) -> Self {
        Self {
            from_schema: f.from_schema,
            from_table: f.from_table,
            from_col: f.from_col,
            to_schema: f.to_schema,
            to_table: f.to_table,
            to_col: f.to_col,
            constraint_name: f.constraint_name,
        }
    }
}

// ---------------------------------------------------------------------------
// DiagramData → FfiDiagramData
// ---------------------------------------------------------------------------

impl From<sbql_core::DiagramData> for FfiDiagramData {
    fn from(d: sbql_core::DiagramData) -> Self {
        Self {
            tables: d.tables.into_iter().map(Into::into).collect(),
            foreign_keys: d.foreign_keys.into_iter().map(Into::into).collect(),
        }
    }
}
