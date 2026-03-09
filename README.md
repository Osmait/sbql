# sbql

`sbql` is a multi-platform SQL workspace built with Rust. It provides a terminal UI and a native macOS app for managing database connections, running queries, and visualizing schemas.

![License](https://img.shields.io/crates/l/sbql-core)
![Rust](https://img.shields.io/badge/rust-1.70%2B-blue.svg)

## Features

- **Multi-Database Support:** PostgreSQL, SQLite, and Redis.
- **Connection Management:** Manage multiple database connections. Passwords are securely stored using the system keyring.
- **Full SQL Editor:** Write and execute SQL queries with tree-sitter syntax highlighting and autocomplete.
- **Interactive Results View:** View query results with pagination, sorting (via SQL AST injection), and filtering.
- **Direct Cell Editing:** Edit cells or delete rows directly from the results view.
- **Schema Browser:** List tables, view columns, primary keys, and foreign keys.
- **Diagram View:** Visualize database schema and relationships directly in the terminal or macOS app.
- **Auto-Reconnect:** Automatically reconnects to the last used database on startup.
- **Native macOS App:** Full-featured SwiftUI app powered by the same Rust core via UniFFI.

## Architecture

```mermaid
graph TB
    subgraph "User Interfaces"
        TUI["sbql-tui<br/>(Terminal UI)"]
        macOS["sbql-macos<br/>(SwiftUI App)"]
    end
    subgraph "Bridge"
        FFI["sbql-ffi<br/>(UniFFI)"]
    end
    subgraph "Core"
        Core["sbql-core<br/>(Headless Engine)"]
    end
    subgraph "Databases"
        PG[(PostgreSQL)]
        SQLite[(SQLite)]
        Redis[(Redis)]
    end
    TUI -->|"CoreCommand / CoreEvent"| Core
    macOS --> FFI --> Core
    Core --> PG
    Core --> SQLite
    Core --> Redis
```

The project follows a **headless core** pattern:

- **`sbql-core`:** UI-agnostic library handling connections, query execution, SQL AST manipulation, and schema introspection. Communicates via `CoreCommand`/`CoreEvent`.
- **`sbql-tui`:** Terminal interface built with Ratatui. Uses a background worker thread for non-blocking operations.
- **`sbql-ffi`:** UniFFI bridge exposing the core to Swift as a C FFI static library + XCFramework.
- **`sbql-macos`:** Native macOS app built with SwiftUI following MVVM architecture.

For detailed architecture documentation with diagrams, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Prerequisites

- [Rust toolchain](https://rustup.rs/) (edition 2021)
- A running database (PostgreSQL, SQLite, or Redis)
- For macOS app: Xcode 15+ with Swift 6

## Installation

### Terminal UI

```bash
# From source
cargo build --release
# Binary at target/release/sbql

# Or install directly
make install-local
```

### macOS App

```bash
# Build XCFramework + install app
make xcframework
make install-macos
```

## Usage

```bash
# Run the TUI
sbql

# Or via cargo
cargo run -p sbql-tui
```

### Keybindings

| Key | Action |
|-----|--------|
| `Tab` | Cycle between views (Editor, Results, Connections, Schema, Diagram) |
| `Ctrl+E` | Execute query |
| Arrow keys | Navigate tables and lists |
| `Enter` | Select / edit cell |
| `d` | Mark row for deletion (in Results) |
| `s` | Sort by column |
| `/` | Open filter bar |
| `?` | Show help |

## Dependencies

- **[Ratatui](https://github.com/ratatui/ratatui):** Terminal UI framework
- **[SQLx](https://github.com/launchbadge/sqlx):** Async database driver (PostgreSQL, SQLite)
- **[redis-rs](https://github.com/redis-rs/redis-rs):** Redis client
- **[sqlparser-rs](https://github.com/sqlparser-rs/sqlparser-rs):** SQL AST parser for query manipulation
- **[tree-sitter](https://tree-sitter.github.io/):** Syntax highlighting
- **[Tokio](https://github.com/tokio-rs/tokio):** Async runtime
- **[UniFFI](https://github.com/mozilla/uniffi-rs):** Rust ↔ Swift FFI bridge
- **[Keyring](https://github.com/hwchen/keyring-rs):** Secure credential storage

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for architecture details, development workflow, and how to get started.

## License

This project is licensed under the MIT License.
