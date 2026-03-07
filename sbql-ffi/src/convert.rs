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
            sbql_core::DbBackend::Redis => FfiDbBackend::Redis,
        }
    }
}

impl From<FfiDbBackend> for sbql_core::DbBackend {
    fn from(b: FfiDbBackend) -> Self {
        match b {
            FfiDbBackend::Postgres => sbql_core::DbBackend::Postgres,
            FfiDbBackend::Sqlite => sbql_core::DbBackend::Sqlite,
            FfiDbBackend::Redis => sbql_core::DbBackend::Redis,
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

#[cfg(test)]
mod tests {
    use super::*;

    // --- DbBackend ↔ FfiDbBackend ---

    #[test]
    fn db_backend_roundtrip_postgres() {
        let ffi: FfiDbBackend = sbql_core::DbBackend::Postgres.into();
        assert!(matches!(ffi, FfiDbBackend::Postgres));
        let back: sbql_core::DbBackend = ffi.into();
        assert_eq!(back, sbql_core::DbBackend::Postgres);
    }

    #[test]
    fn db_backend_roundtrip_sqlite() {
        let ffi: FfiDbBackend = sbql_core::DbBackend::Sqlite.into();
        assert!(matches!(ffi, FfiDbBackend::Sqlite));
        let back: sbql_core::DbBackend = ffi.into();
        assert_eq!(back, sbql_core::DbBackend::Sqlite);
    }

    // --- SslMode ↔ FfiSslMode ---

    #[test]
    fn ssl_mode_roundtrip_all_variants() {
        let variants = vec![
            (sbql_core::SslMode::Prefer, FfiSslMode::Prefer),
            (sbql_core::SslMode::Disable, FfiSslMode::Disable),
            (sbql_core::SslMode::Require, FfiSslMode::Require),
            (sbql_core::SslMode::VerifyCa, FfiSslMode::VerifyCa),
            (sbql_core::SslMode::VerifyFull, FfiSslMode::VerifyFull),
        ];
        for (core_val, expected_ffi) in variants {
            let ffi: FfiSslMode = core_val.clone().into();
            assert_eq!(format!("{ffi:?}"), format!("{expected_ffi:?}"));
            let back: sbql_core::SslMode = ffi.into();
            assert_eq!(back, core_val);
        }
    }

    // --- SortDirection ---

    #[test]
    fn sort_direction_from_ffi() {
        let asc: sbql_core::SortDirection = FfiSortDirection::Ascending.into();
        assert_eq!(asc, sbql_core::SortDirection::Ascending);
        let desc: sbql_core::SortDirection = FfiSortDirection::Descending.into();
        assert_eq!(desc, sbql_core::SortDirection::Descending);
    }

    // --- ConnectionConfig ↔ FfiConnectionConfig ---

    #[test]
    fn connection_config_to_ffi_all_fields() {
        let id = uuid::Uuid::new_v4();
        let config = sbql_core::ConnectionConfig {
            id,
            name: "test".into(),
            backend: sbql_core::DbBackend::Postgres,
            host: "localhost".into(),
            port: 5432,
            user: "admin".into(),
            database: "mydb".into(),
            ssl_mode: sbql_core::SslMode::Require,
            file_path: Some("/tmp/test.db".into()),
        };
        let ffi: FfiConnectionConfig = config.into();
        assert_eq!(ffi.id, id.to_string());
        assert_eq!(ffi.name, "test");
        assert!(matches!(ffi.backend, FfiDbBackend::Postgres));
        assert_eq!(ffi.host, "localhost");
        assert_eq!(ffi.port, 5432);
        assert_eq!(ffi.user, "admin");
        assert_eq!(ffi.database, "mydb");
        assert!(matches!(ffi.ssl_mode, FfiSslMode::Require));
        assert_eq!(ffi.file_path, Some("/tmp/test.db".to_string()));
    }

    #[test]
    fn ffi_connection_config_try_from_valid_uuid() {
        let id = uuid::Uuid::new_v4();
        let ffi = FfiConnectionConfig {
            id: id.to_string(),
            name: "test".into(),
            backend: FfiDbBackend::Sqlite,
            host: "".into(),
            port: 0,
            user: "".into(),
            database: "".into(),
            ssl_mode: FfiSslMode::Prefer,
            file_path: None,
        };
        let config: Result<sbql_core::ConnectionConfig, _> = ffi.try_into();
        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.id, id);
        assert_eq!(config.backend, sbql_core::DbBackend::Sqlite);
    }

    #[test]
    fn ffi_connection_config_try_from_invalid_uuid() {
        let ffi = FfiConnectionConfig {
            id: "not-a-uuid".into(),
            name: "test".into(),
            backend: FfiDbBackend::Postgres,
            host: "".into(),
            port: 0,
            user: "".into(),
            database: "".into(),
            ssl_mode: FfiSslMode::Prefer,
            file_path: None,
        };
        let result: Result<sbql_core::ConnectionConfig, SbqlFfiError> = ffi.try_into();
        assert!(result.is_err());
        match result.unwrap_err() {
            SbqlFfiError::InvalidArgument { msg } => assert!(msg.contains("Invalid UUID")),
            _ => panic!("Expected InvalidArgument"),
        }
    }

    // --- QueryResult → FfiQueryResult ---

    #[test]
    fn query_result_to_ffi() {
        let qr = sbql_core::QueryResult {
            columns: vec!["id".into(), "name".into()],
            rows: vec![vec!["1".into(), "Alice".into()]],
            page: 3,
            has_next_page: true,
        };
        let ffi: FfiQueryResult = qr.into();
        assert_eq!(ffi.columns, vec!["id", "name"]);
        assert_eq!(ffi.rows, vec![vec!["1", "Alice"]]);
        assert_eq!(ffi.page, 3);
        assert!(ffi.has_next_page);
    }

    // --- TableEntry → FfiTableEntry ---

    #[test]
    fn table_entry_to_ffi() {
        let te = sbql_core::TableEntry {
            schema: "public".into(),
            name: "users".into(),
        };
        let ffi: FfiTableEntry = te.into();
        assert_eq!(ffi.schema, "public");
        assert_eq!(ffi.name, "users");
    }

    // --- ColumnInfo → FfiColumnInfo ---

    #[test]
    fn column_info_to_ffi() {
        let ci = sbql_core::ColumnInfo {
            name: "id".into(),
            data_type: "integer".into(),
            is_pk: true,
            is_nullable: false,
        };
        let ffi: FfiColumnInfo = ci.into();
        assert_eq!(ffi.name, "id");
        assert_eq!(ffi.data_type, "integer");
        assert!(ffi.is_pk);
        assert!(!ffi.is_nullable);
    }

    // --- TableSchema → FfiTableSchema ---

    #[test]
    fn table_schema_to_ffi() {
        let ts = sbql_core::TableSchema {
            schema: "public".into(),
            name: "users".into(),
            columns: vec![sbql_core::ColumnInfo {
                name: "id".into(),
                data_type: "integer".into(),
                is_pk: true,
                is_nullable: false,
            }],
        };
        let ffi: FfiTableSchema = ts.into();
        assert_eq!(ffi.schema, "public");
        assert_eq!(ffi.name, "users");
        assert_eq!(ffi.columns.len(), 1);
        assert_eq!(ffi.columns[0].name, "id");
    }

    // --- ForeignKey → FfiForeignKey ---

    #[test]
    fn foreign_key_to_ffi() {
        let fk = sbql_core::ForeignKey {
            from_schema: "public".into(),
            from_table: "orders".into(),
            from_col: "user_id".into(),
            to_schema: "public".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            constraint_name: "fk_orders_users".into(),
        };
        let ffi: FfiForeignKey = fk.into();
        assert_eq!(ffi.from_schema, "public");
        assert_eq!(ffi.from_table, "orders");
        assert_eq!(ffi.from_col, "user_id");
        assert_eq!(ffi.to_schema, "public");
        assert_eq!(ffi.to_table, "users");
        assert_eq!(ffi.to_col, "id");
        assert_eq!(ffi.constraint_name, "fk_orders_users");
    }

    // --- DiagramData → FfiDiagramData ---

    #[test]
    fn diagram_data_to_ffi() {
        let dd = sbql_core::DiagramData {
            tables: vec![sbql_core::TableSchema {
                schema: "public".into(),
                name: "users".into(),
                columns: vec![sbql_core::ColumnInfo {
                    name: "id".into(),
                    data_type: "integer".into(),
                    is_pk: true,
                    is_nullable: false,
                }],
            }],
            foreign_keys: vec![sbql_core::ForeignKey {
                from_schema: "public".into(),
                from_table: "orders".into(),
                from_col: "user_id".into(),
                to_schema: "public".into(),
                to_table: "users".into(),
                to_col: "id".into(),
                constraint_name: "fk_test".into(),
            }],
        };
        let ffi: FfiDiagramData = dd.into();
        assert_eq!(ffi.tables.len(), 1);
        assert_eq!(ffi.foreign_keys.len(), 1);
        assert_eq!(ffi.tables[0].name, "users");
        assert_eq!(ffi.foreign_keys[0].constraint_name, "fk_test");
    }
}
