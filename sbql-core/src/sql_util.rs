//! Shared SQL identifier quoting helpers.
//!
//! Each function escapes the delimiter character by doubling it, then wraps the
//! identifier in the appropriate quote characters for the target backend.

/// Quote a PostgreSQL/SQLite identifier (doubles `"`).
pub fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Quote a MySQL identifier (doubles `` ` ``).
pub fn quote_ident_mysql(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

/// Quote a SQL Server identifier (doubles `]`).
pub fn quote_ident_sqlserver(name: &str) -> String {
    format!("[{}]", name.replace(']', "]]"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pg_simple() {
        assert_eq!(quote_ident("users"), "\"users\"");
    }

    #[test]
    fn pg_with_double_quote() {
        assert_eq!(quote_ident("col\"name"), "\"col\"\"name\"");
    }

    #[test]
    fn mysql_simple() {
        assert_eq!(quote_ident_mysql("users"), "`users`");
    }

    #[test]
    fn mysql_with_backtick() {
        assert_eq!(quote_ident_mysql("col`name"), "`col``name`");
    }

    #[test]
    fn sqlserver_simple() {
        assert_eq!(quote_ident_sqlserver("users"), "[users]");
    }

    #[test]
    fn sqlserver_with_bracket() {
        assert_eq!(quote_ident_sqlserver("col]name"), "[col]]name]");
    }
}
