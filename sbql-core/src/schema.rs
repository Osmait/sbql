//! Schema introspection via `information_schema` (PG) or `sqlite_master` + PRAGMAs (SQLite).

use sqlx::{PgPool, Row, SqlitePool};

use crate::error::{Result, SbqlError};
use crate::pool::DbPool;

/// A table entry returned by schema introspection.
#[derive(Debug, Clone)]
pub struct TableEntry {
    pub schema: String,
    pub name: String,
}

impl TableEntry {
    /// Qualified name suitable for display: `schema.name`.
    pub fn qualified(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
}

// ---------------------------------------------------------------------------
// Diagram data types
// ---------------------------------------------------------------------------

/// A single column in a table, with metadata for diagram rendering.
#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub is_pk: bool,
    pub is_nullable: bool,
}

/// Full schema of one table (used for the diagram).
#[derive(Debug, Clone)]
pub struct TableSchema {
    pub schema: String,
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

impl TableSchema {
    pub fn qualified(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }
}

/// A foreign-key relationship between two tables.
#[derive(Debug, Clone)]
pub struct ForeignKey {
    /// Source table schema
    pub from_schema: String,
    /// Source table name
    pub from_table: String,
    /// Source column(s) (comma-joined for display)
    pub from_col: String,
    /// Referenced table schema
    pub to_schema: String,
    /// Referenced table name
    pub to_table: String,
    /// Referenced column(s)
    pub to_col: String,
    /// Constraint name
    pub constraint_name: String,
}

/// Everything needed to render the database diagram.
#[derive(Debug, Clone, Default)]
pub struct DiagramData {
    pub tables: Vec<TableSchema>,
    pub foreign_keys: Vec<ForeignKey>,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// List all user-visible tables.
pub async fn list_tables(pool: &DbPool) -> Result<Vec<TableEntry>> {
    match pool {
        DbPool::Postgres(pg) => list_tables_pg(pg).await,
        DbPool::Sqlite(sq) => list_tables_sqlite(sq).await,
        DbPool::Redis(_) => Ok(vec![]),
    }
}

/// Return the primary key column name(s) for a given table.
pub async fn get_primary_keys(pool: &DbPool, schema: &str, table: &str) -> Result<Vec<String>> {
    match pool {
        DbPool::Postgres(pg) => get_primary_keys_pg(pg, schema, table).await,
        DbPool::Sqlite(sq) => get_primary_keys_sqlite(sq, table).await,
        DbPool::Redis(_) => Ok(vec![]),
    }
}

/// Load all table schemas + FK relationships for the diagram view.
pub async fn load_diagram(pool: &DbPool) -> Result<DiagramData> {
    match pool {
        DbPool::Postgres(pg) => load_diagram_pg(pg).await,
        DbPool::Sqlite(sq) => load_diagram_sqlite(sq).await,
        DbPool::Redis(_) => Ok(DiagramData::default()),
    }
}

/// Execute a single-cell UPDATE.
pub async fn execute_cell_update(
    pool: &DbPool,
    schema: &str,
    table: &str,
    pk_col: &str,
    pk_val: &str,
    target_col: &str,
    new_val: &str,
) -> Result<()> {
    match pool {
        DbPool::Postgres(pg) => {
            execute_cell_update_pg(pg, schema, table, pk_col, pk_val, target_col, new_val).await
        }
        DbPool::Sqlite(sq) => {
            execute_cell_update_sqlite(sq, table, pk_col, pk_val, target_col, new_val).await
        }
        DbPool::Redis(_) => Err(SbqlError::Schema(
            "Cell update not supported for Redis".into(),
        )),
    }
}

/// Execute a single-row DELETE identified by its primary key.
pub async fn execute_row_delete(
    pool: &DbPool,
    schema: &str,
    table: &str,
    pk_col: &str,
    pk_val: &str,
) -> Result<()> {
    match pool {
        DbPool::Postgres(pg) => execute_row_delete_pg(pg, schema, table, pk_col, pk_val).await,
        DbPool::Sqlite(sq) => execute_row_delete_sqlite(sq, table, pk_col, pk_val).await,
        DbPool::Redis(_) => Err(SbqlError::Schema(
            "Row delete not supported for Redis".into(),
        )),
    }
}

// ---------------------------------------------------------------------------
// PostgreSQL implementations
// ---------------------------------------------------------------------------

async fn list_tables_pg(pool: &PgPool) -> Result<Vec<TableEntry>> {
    let rows = sqlx::query(
        r#"
        SELECT table_schema, table_name
        FROM information_schema.tables
        WHERE table_type IN ('BASE TABLE', 'VIEW')
          AND table_schema NOT IN ('pg_catalog', 'information_schema')
        ORDER BY table_schema, table_name
        "#,
    )
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        tracing::warn!(
            "list_tables_pg: information_schema returned 0 rows, trying pg_catalog fallback"
        );
        // Fallback: use pg_catalog which is always accessible
        let fallback_rows = sqlx::query(
            r#"
            SELECT n.nspname AS table_schema, c.relname AS table_name
            FROM pg_catalog.pg_class c
            JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
            WHERE c.relkind IN ('r', 'v', 'm', 'p')
              AND n.nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
              AND has_table_privilege(c.oid, 'SELECT')
            ORDER BY n.nspname, c.relname
            "#,
        )
        .fetch_all(pool)
        .await?;

        tracing::debug!(
            "list_tables_pg: pg_catalog fallback returned {} rows",
            fallback_rows.len()
        );

        return Ok(fallback_rows
            .into_iter()
            .map(|r| TableEntry {
                schema: r
                    .try_get::<String, _>("table_schema")
                    .unwrap_or_else(|_| "public".into()),
                name: r.try_get::<String, _>("table_name").unwrap_or_default(),
            })
            .collect());
    }

    Ok(rows
        .into_iter()
        .map(|r| TableEntry {
            schema: r
                .try_get::<String, _>("table_schema")
                .unwrap_or_else(|_| "public".into()),
            name: r.try_get::<String, _>("table_name").unwrap_or_default(),
        })
        .collect())
}

async fn get_primary_keys_pg(pool: &PgPool, schema: &str, table: &str) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
        SELECT kcu.column_name
        FROM information_schema.key_column_usage AS kcu
        JOIN information_schema.table_constraints AS tc
          ON tc.constraint_name = kcu.constraint_name
         AND tc.table_schema    = kcu.table_schema
         AND tc.table_name      = kcu.table_name
        WHERE tc.constraint_type = 'PRIMARY KEY'
          AND kcu.table_schema   = $1
          AND kcu.table_name     = $2
        ORDER BY kcu.ordinal_position
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;

    let pks: Vec<String> = rows
        .into_iter()
        .filter_map(|r| r.try_get::<String, _>("column_name").ok())
        .collect();

    if pks.is_empty() {
        return Err(SbqlError::Schema(format!(
            "No primary key found for {schema}.{table}"
        )));
    }

    Ok(pks)
}

async fn load_diagram_pg(pool: &PgPool) -> Result<DiagramData> {
    // --- Columns with PK flag ---
    let col_rows = sqlx::query(
        r#"
        SELECT
            c.table_schema,
            c.table_name,
            c.column_name,
            c.data_type,
            c.is_nullable,
            CASE WHEN pk.column_name IS NOT NULL THEN true ELSE false END AS is_pk
        FROM information_schema.columns c
        LEFT JOIN (
            SELECT kcu.table_schema, kcu.table_name, kcu.column_name
            FROM information_schema.key_column_usage kcu
            JOIN information_schema.table_constraints tc
              ON tc.constraint_name = kcu.constraint_name
             AND tc.table_schema    = kcu.table_schema
             AND tc.table_name      = kcu.table_name
            WHERE tc.constraint_type = 'PRIMARY KEY'
        ) pk
          ON pk.table_schema  = c.table_schema
         AND pk.table_name    = c.table_name
         AND pk.column_name   = c.column_name
        WHERE c.table_schema NOT IN ('pg_catalog', 'information_schema')
          AND EXISTS (
            SELECT 1 FROM information_schema.tables t
            WHERE t.table_schema = c.table_schema
              AND t.table_name   = c.table_name
              AND t.table_type   = 'BASE TABLE'
          )
        ORDER BY c.table_schema, c.table_name, c.ordinal_position
        "#,
    )
    .fetch_all(pool)
    .await?;

    // Build table map
    let mut table_map: indexmap::IndexMap<(String, String), Vec<ColumnInfo>> =
        indexmap::IndexMap::new();
    for row in col_rows {
        let ts: String = row.try_get("table_schema").unwrap_or_default();
        let tn: String = row.try_get("table_name").unwrap_or_default();
        let col = ColumnInfo {
            name: row.try_get("column_name").unwrap_or_default(),
            data_type: row.try_get("data_type").unwrap_or_default(),
            is_pk: row.try_get("is_pk").unwrap_or(false),
            is_nullable: row
                .try_get::<String, _>("is_nullable")
                .map(|s| s == "YES")
                .unwrap_or(true),
        };
        table_map.entry((ts, tn)).or_default().push(col);
    }

    let tables: Vec<TableSchema> = table_map
        .into_iter()
        .map(|((schema, name), columns)| TableSchema {
            schema,
            name,
            columns,
        })
        .collect();

    // --- Foreign keys ---
    let fk_rows = sqlx::query(
        r#"
        SELECT
            tc.constraint_name,
            tc.table_schema  AS from_schema,
            tc.table_name    AS from_table,
            kcu.column_name  AS from_col,
            ccu.table_schema AS to_schema,
            ccu.table_name   AS to_table,
            ccu.column_name  AS to_col
        FROM information_schema.table_constraints tc
        JOIN information_schema.key_column_usage kcu
          ON kcu.constraint_name = tc.constraint_name
         AND kcu.table_schema    = tc.table_schema
        JOIN information_schema.constraint_column_usage ccu
          ON ccu.constraint_name = tc.constraint_name
        WHERE tc.constraint_type = 'FOREIGN KEY'
          AND tc.table_schema NOT IN ('pg_catalog', 'information_schema')
        ORDER BY tc.table_schema, tc.table_name, tc.constraint_name
        "#,
    )
    .fetch_all(pool)
    .await?;

    let foreign_keys: Vec<ForeignKey> = fk_rows
        .into_iter()
        .map(|row| ForeignKey {
            constraint_name: row.try_get("constraint_name").unwrap_or_default(),
            from_schema: row.try_get("from_schema").unwrap_or_default(),
            from_table: row.try_get("from_table").unwrap_or_default(),
            from_col: row.try_get("from_col").unwrap_or_default(),
            to_schema: row.try_get("to_schema").unwrap_or_default(),
            to_table: row.try_get("to_table").unwrap_or_default(),
            to_col: row.try_get("to_col").unwrap_or_default(),
        })
        .collect();

    Ok(DiagramData {
        tables,
        foreign_keys,
    })
}

async fn execute_cell_update_pg(
    pool: &PgPool,
    schema: &str,
    table: &str,
    pk_col: &str,
    pk_val: &str,
    target_col: &str,
    new_val: &str,
) -> Result<()> {
    let target_type = resolve_column_type(pool, schema, table, target_col).await?;
    let sql = format!(
        r#"UPDATE "{schema}"."{table}" SET "{target_col}" = $1::{target_type} WHERE "{pk_col}"::text = $2"#
    );
    sqlx::query(&sql)
        .bind(new_val)
        .bind(pk_val)
        .execute(pool)
        .await?;
    Ok(())
}

async fn execute_row_delete_pg(
    pool: &PgPool,
    schema: &str,
    table: &str,
    pk_col: &str,
    pk_val: &str,
) -> Result<()> {
    let sql = format!(r#"DELETE FROM "{schema}"."{table}" WHERE "{pk_col}"::text = $1"#);
    sqlx::query(&sql).bind(pk_val).execute(pool).await?;
    Ok(())
}

async fn resolve_column_type(
    pool: &PgPool,
    schema: &str,
    table: &str,
    column: &str,
) -> Result<String> {
    let row = sqlx::query(
        r#"
        SELECT pg_catalog.format_type(a.atttypid, a.atttypmod) AS type_name
        FROM pg_catalog.pg_attribute a
        JOIN pg_catalog.pg_class c ON c.oid = a.attrelid
        JOIN pg_catalog.pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = $1
          AND c.relname = $2
          AND a.attname = $3
          AND a.attnum > 0
          AND NOT a.attisdropped
        LIMIT 1
        "#,
    )
    .bind(schema)
    .bind(table)
    .bind(column)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Err(SbqlError::Schema(format!(
            "Column type not found for {schema}.{table}.{column}"
        )));
    };

    let type_name: String = row.try_get("type_name").unwrap_or_default();
    if type_name.is_empty() {
        return Err(SbqlError::Schema(format!(
            "Empty column type for {schema}.{table}.{column}"
        )));
    }

    Ok(type_name)
}

// ---------------------------------------------------------------------------
// SQLite implementations
// ---------------------------------------------------------------------------

async fn list_tables_sqlite(pool: &SqlitePool) -> Result<Vec<TableEntry>> {
    let rows = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| TableEntry {
            schema: "main".to_string(),
            name: r.try_get::<String, _>("name").unwrap_or_default(),
        })
        .collect())
}

async fn get_primary_keys_sqlite(pool: &SqlitePool, table: &str) -> Result<Vec<String>> {
    let sql = format!("PRAGMA table_info(\"{}\")", table.replace('"', "\"\""));
    let rows = sqlx::query(&sql).fetch_all(pool).await?;

    let mut pks: Vec<(i32, String)> = Vec::new();
    for row in rows {
        let pk: i32 = row.try_get("pk").unwrap_or(0);
        if pk > 0 {
            let name: String = row.try_get("name").unwrap_or_default();
            pks.push((pk, name));
        }
    }
    pks.sort_by_key(|(pk, _)| *pk);
    let pks: Vec<String> = pks.into_iter().map(|(_, name)| name).collect();

    if pks.is_empty() {
        return Err(SbqlError::Schema(format!(
            "No primary key found for {table}"
        )));
    }

    Ok(pks)
}

async fn load_diagram_sqlite(pool: &SqlitePool) -> Result<DiagramData> {
    // 1. Get all tables
    let table_rows = sqlx::query(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(pool)
    .await?;

    let table_names: Vec<String> = table_rows
        .into_iter()
        .filter_map(|r| r.try_get::<String, _>("name").ok())
        .collect();

    let mut tables = Vec::new();
    let mut foreign_keys = Vec::new();

    for table_name in &table_names {
        // 2. Columns from PRAGMA table_info
        let col_sql = format!("PRAGMA table_info(\"{}\")", table_name.replace('"', "\"\""));
        let col_rows = sqlx::query(&col_sql).fetch_all(pool).await?;

        let columns: Vec<ColumnInfo> = col_rows
            .into_iter()
            .map(|r| {
                let pk: i32 = r.try_get("pk").unwrap_or(0);
                let notnull: bool = r.try_get("notnull").unwrap_or(false);
                ColumnInfo {
                    name: r.try_get("name").unwrap_or_default(),
                    data_type: r.try_get("type").unwrap_or_default(),
                    is_pk: pk > 0,
                    is_nullable: !notnull && pk == 0,
                }
            })
            .collect();

        tables.push(TableSchema {
            schema: "main".to_string(),
            name: table_name.clone(),
            columns,
        });

        // 3. Foreign keys from PRAGMA foreign_key_list
        let fk_sql = format!(
            "PRAGMA foreign_key_list(\"{}\")",
            table_name.replace('"', "\"\"")
        );
        let fk_rows = sqlx::query(&fk_sql).fetch_all(pool).await?;

        for fk_row in fk_rows {
            let id: i32 = fk_row.try_get("id").unwrap_or(0);
            foreign_keys.push(ForeignKey {
                from_schema: "main".to_string(),
                from_table: table_name.clone(),
                from_col: fk_row.try_get("from").unwrap_or_default(),
                to_schema: "main".to_string(),
                to_table: fk_row.try_get("table").unwrap_or_default(),
                to_col: fk_row.try_get("to").unwrap_or_default(),
                constraint_name: format!("fk_{table_name}_{id}"),
            });
        }
    }

    Ok(DiagramData {
        tables,
        foreign_keys,
    })
}

async fn execute_cell_update_sqlite(
    pool: &SqlitePool,
    table: &str,
    pk_col: &str,
    pk_val: &str,
    target_col: &str,
    new_val: &str,
) -> Result<()> {
    let sql = format!(r#"UPDATE "{table}" SET "{target_col}" = $1 WHERE "{pk_col}" = $2"#);
    sqlx::query(&sql)
        .bind(new_val)
        .bind(pk_val)
        .execute(pool)
        .await?;
    Ok(())
}

async fn execute_row_delete_sqlite(
    pool: &SqlitePool,
    table: &str,
    pk_col: &str,
    pk_val: &str,
) -> Result<()> {
    let sql = format!(r#"DELETE FROM "{table}" WHERE "{pk_col}" = $1"#);
    sqlx::query(&sql).bind(pk_val).execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_update_sql(schema: &str, table: &str, pk_col: &str, target_col: &str) -> String {
        format!(r#"UPDATE "{schema}"."{table}" SET "{target_col}" = $1 WHERE "{pk_col}" = $2"#)
    }

    #[test]
    fn test_table_entry_qualified() {
        let entry = TableEntry {
            schema: "public".to_string(),
            name: "users".to_string(),
        };
        assert_eq!(entry.qualified(), "public.users");
    }

    #[test]
    fn test_table_schema_qualified() {
        let ts = TableSchema {
            schema: "auth".to_string(),
            name: "sessions".to_string(),
            columns: vec![],
        };
        assert_eq!(ts.qualified(), "auth.sessions");
    }

    #[test]
    fn test_build_update_sql() {
        let sql = build_update_sql("public", "users", "id", "email");
        assert_eq!(
            sql,
            r#"UPDATE "public"."users" SET "email" = $1 WHERE "id" = $2"#
        );
    }

    // --- Phase 1E: additional edge-case tests ---

    #[test]
    fn test_build_update_sql_with_double_quotes() {
        let sql = build_update_sql("my\"schema", "my\"table", "pk\"col", "target\"col");
        // Double-quote chars in identifiers are embedded as-is via format!
        assert!(sql.contains(r#""my"schema""#));
        assert!(sql.contains(r#""my"table""#));
        assert!(sql.contains(r#""target"col""#));
        assert!(sql.contains(r#""pk"col""#));
    }

    #[test]
    fn test_table_entry_qualified_special_chars() {
        let entry = TableEntry {
            schema: "my.schema".to_string(),
            name: "my table".to_string(),
        };
        assert_eq!(entry.qualified(), "my.schema.my table");
    }

    #[test]
    fn test_table_schema_empty_columns() {
        let ts = TableSchema {
            schema: "public".to_string(),
            name: "empty_table".to_string(),
            columns: vec![],
        };
        assert_eq!(ts.qualified(), "public.empty_table");
        assert!(ts.columns.is_empty());
    }
}
