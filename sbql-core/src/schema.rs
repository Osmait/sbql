//! Schema introspection via `information_schema`.

use sqlx::{PgPool, Row};

use crate::error::{Result, SbqlError};

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
// Queries
// ---------------------------------------------------------------------------

/// List all user-visible tables (excludes pg_catalog, information_schema).
pub async fn list_tables(pool: &PgPool) -> Result<Vec<TableEntry>> {
    let rows = sqlx::query(
        r#"
        SELECT table_schema, table_name
        FROM information_schema.tables
        WHERE table_type = 'BASE TABLE'
          AND table_schema NOT IN ('pg_catalog', 'information_schema')
        ORDER BY table_schema, table_name
        "#,
    )
    .fetch_all(pool)
    .await?;

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

/// Return the primary key column name(s) for a given table.
pub async fn get_primary_keys(pool: &PgPool, schema: &str, table: &str) -> Result<Vec<String>> {
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

/// Load all table schemas + FK relationships for the diagram view.
pub async fn load_diagram(pool: &PgPool) -> Result<DiagramData> {
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

// ---------------------------------------------------------------------------
// Cell update helpers
// ---------------------------------------------------------------------------

/// Build an `UPDATE` statement for a single cell change, using the detected PK.
pub fn build_update_sql(schema: &str, table: &str, pk_col: &str, target_col: &str) -> String {
    format!(r#"UPDATE "{schema}"."{table}" SET "{target_col}" = $1 WHERE "{pk_col}" = $2"#)
}

/// Execute a single-cell UPDATE.
pub async fn execute_cell_update(
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

/// Execute a single-row DELETE identified by its primary key.
pub async fn execute_row_delete(
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
