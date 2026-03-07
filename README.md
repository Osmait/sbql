# sbql

`sbql` is a terminal SQL workspace built with Rust. It provides a robust, terminal-based interface to manage your PostgreSQL databases, run queries, and visualize schemas without leaving your command line.

![License](https://img.shields.io/crates/l/sbql-core)
![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)

## Features

- **Connection Management:** Manage multiple PostgreSQL connections. Passwords are securely stored using the system keyring.
- **Full SQL Editor:** Write and execute complex SQL queries directly from the terminal, powered by `tui-textarea`.
- **Interactive Results View:** View query results in an interactive table with support for pagination, sorting (intelligently injecting logic via SQL AST), and filtering.
- **Direct Cell Editing:** Edit table cells or delete rows directly from the results view, making data manipulation quick and straightforward.
- **Schema Browser:** Easily list database tables and view primary keys.
- **Diagram View:** Visualize your database schema and foreign key relationships directly in the terminal.

## Architecture

The project follows a decoupled architecture, split into two main crates:

- **`sbql-core`:** A UI-agnostic headless core library. It manages database connection pools (via SQLx), parses and securely modifies SQL queries (via sqlparser), and handles the core application state machine. It communicates asynchronously via `CoreCommand` and `CoreEvent`.
- **`sbql-tui`:** The terminal user interface built with `ratatui`. It uses a background worker thread to process core commands asynchronously, ensuring the UI remains responsive even during heavy queries.

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (edition 2021)
- A running PostgreSQL database

## Installation

To build the project from source, clone the repository and run:

```bash
cargo build --release
```

The compiled binary will be available at `target/release/sbql`.

## Usage

To run the terminal UI directly using Cargo:

```bash
cargo run -p sbql-tui
```

Alternatively, if you've installed it or added the binary to your PATH, simply run:

```bash
sbql
```

### Keybindings

*(Depending on the current view within the TUI)*

- Navigate between views: Editor, Results, Connections, Schema, and Diagram.
- Use arrow keys to navigate tables and lists.
- Press `Enter` to execute queries or edit cells.

## Dependencies

- **[Ratatui](https://github.com/ratatui/ratatui):** Terminal UI framework.
- **[SQLx](https://github.com/launchbadge/sqlx):** Async PostgreSQL driver.
- **[sqlparser-rs](https://github.com/sqlparser-rs/sqlparser-rs):** Extensible SQL lexer and parser.
- **[Tokio](https://github.com/tokio-rs/tokio):** Asynchronous runtime.
- **[Keyring](https://github.com/hwchen/keyring-rs):** Cross-platform secure credential storage.

## License

This project is licensed under the MIT License.

---

