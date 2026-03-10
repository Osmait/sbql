# sbql

`sbql` is an open-source SQL workspace built around a headless Rust core.

It gives you two ways to work with the same engine:

- a keyboard-first terminal UI for fast daily querying
- a native macOS app for a more visual workflow

Today the project supports PostgreSQL, SQLite, and Redis in the core and TUI, with PostgreSQL and SQLite exposed in the macOS app.

![License](https://img.shields.io/crates/l/sbql-core)
![Rust](https://img.shields.io/badge/rust-stable-blue.svg)

## Why sbql?

Most database tools force a trade-off:

- terminal speed, but weak UX
- polished GUI, but no reusable engine
- one-off query runners, but no schema or editing workflow

`sbql` is designed as a shared workspace engine first. The Rust core owns connections, query execution, pagination, SQL AST rewriting, schema introspection, and data mutations. The TUI and macOS app sit on top of that same core, so features can be shared instead of reimplemented.

## What it does

### Core capabilities

- Save and reuse named connections
- Store passwords in the system keyring
- Execute SQL with server-side pagination
- Apply sorting and filtering through SQL AST manipulation instead of string concatenation
- Introspect tables, columns, primary keys, and foreign-key relationships
- Build schema diagram data for visual exploration
- Update individual cells and delete rows for supported SQL backends
- Execute Redis commands and normalize the response into tabular results

### TUI experience

- Multi-panel workspace for Connections, Tables, Editor, Results, and Diagram
- Tree-sitter SQL highlighting and autocomplete
- Keyboard-first navigation with mouse support
- Filter suggestions and staged result editing/deletion workflows
- Full-screen schema diagram with search, focus mode, scrolling, and ASCII/Unicode rendering

### macOS experience

- Native SwiftUI app powered by the same Rust engine through UniFFI
- SQL editor with highlighting, autocomplete popup, undo support, and `Cmd+Enter` execution
- Tabbed results with pagination, dirty-state tracking, and commit/discard actions
- Diagram canvas with drag, zoom, and fit-to-screen behavior
- Auto-reconnect to the last active connection

## Compatibility

### Database support

| Capability | PostgreSQL | SQLite | Redis |
|---|---:|---:|---:|
| Connect and save profiles | Yes | Yes | Yes |
| Query execution | Yes | Yes | Yes |
| Paginated results | Yes | Yes | N/A |
| AST-based sorting | Yes | Yes | No |
| AST-based filtering | Yes | Yes | No |
| Distinct-value suggestions for filters | Yes | Yes | No |
| Schema browser | Yes | Yes | No |
| Diagram data | Yes | Yes | No |
| Cell updates | Yes | Yes | No |
| Row deletes | Yes | Yes | No |

### Frontend support

| Feature | `sbql-tui` | `sbql-macos` |
|---|---:|---:|
| PostgreSQL | Yes | Yes |
| SQLite | Yes | Yes |
| Redis | Yes | Not exposed in UI yet |
| Connection management | Yes | Yes |
| SQL editor | Yes | Yes |
| Result paging, sorting, filtering | Yes | Yes |
| Cell editing and row deletion | Yes | Yes |
| Schema browser | Yes | Yes |
| Diagram view | Yes | Yes |

### Platform support

| Target | Status |
|---|---|
| Rust workspace / TUI | Built and tested in CI on Ubuntu |
| macOS app | Native SwiftUI app with XCFramework bridge |
| Windows | Not documented yet |
| Linux GUI app | Not available |

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

Workspace layout:

- `sbql-core`: UI-agnostic engine for connections, queries, schema, and mutations
- `sbql-tui`: Ratatui-based terminal workspace
- `sbql-ffi`: UniFFI bridge used by Swift
- `sbql-macos`: native macOS client built with SwiftUI

For a deeper architecture walkthrough, see `CONTRIBUTING.md`.

## Installation

### Prerequisites

- Rust toolchain
- A running PostgreSQL, SQLite, or Redis instance
- Docker if you want to run integration tests or container-backed benchmarks
- Xcode 15+ if you want to build the macOS app

### Build the TUI

```bash
cargo build --release
```

Or install it locally:

```bash
make install-local
```

### Build the macOS app

```bash
make xcframework
make install-macos
```

## Quick start

Run the TUI:

```bash
sbql
```

Or directly from the workspace:

```bash
cargo run -p sbql-tui
```

If you already have saved connections, you can start with one by name:

```bash
cargo run -p sbql-tui -- my-connection
```

## Benchmarks

`sbql` ships with a reproducible benchmark suite in `sbql-core/benches/`.

It covers three layers:

- pure CPU microbenchmarks for SQL rewriting and Redis command parsing
- local SQLite execution and schema workloads
- real PostgreSQL and Redis round-trips through Docker-backed containers

### Methodology

Benchmarking in this repository is designed to measure the engine directly, not UI rendering.

- Framework: Criterion
- Local execution: optimized `cargo bench` builds
- SQLite benchmarks: in-memory database with seeded data
- PostgreSQL benchmarks: disposable container started with `testcontainers`
- Redis benchmarks: disposable container started with `testcontainers`
- Report output: Criterion HTML reports under `target/criterion/report/`

Workloads currently included:

- SQL AST operations: `apply_order`, `clear_order`, `apply_filter`
- Pagination SQL generation
- Redis command tokenization and value normalization
- SQLite query execution, lifecycle flow, schema loading, and scaling
- PostgreSQL query execution, schema loading, primary-key lookup, and filter suggestions
- Redis GET/SET/hash/list/bulk command paths

### Reproduce the benchmarks

Run local CPU and SQLite benchmarks:

```bash
make bench
```

Run PostgreSQL integration benchmarks:

```bash
make bench-pg
```

Run Redis integration benchmarks:

```bash
make bench-redis
```

Run everything:

```bash
make bench-all
```

Open the Criterion HTML report:

```bash
make bench-report
```

Memory profiling workload:

```bash
make profile-memory
```

CPU flamegraph:

```bash
make flamegraph
```

### Reference run

The following numbers were collected from this repository on:

- macOS 26.3.1
- Apple M3 Pro (`arm64`)
- `rustc 1.94.0`
- Docker 27.4.0

These numbers should be treated as directional, not contractual. Network stack behavior, Docker overhead, CPU governor, and local system load will move them.

#### Core microbenchmarks

| Benchmark | Result |
|---|---:|
| `build_paginated_sql/page_0` | ~76 ns |
| `tokenize_redis_command/simple` | ~70 ns |
| `apply_filter/column_specific_sqlite` | ~240 ns |
| `apply_filter/global_pg` | ~913 ns |
| `apply_order/postgres_asc` | ~2.60 us |
| `redis_value_to_query_result/large_array` | ~3.17 us |
| `clear_order/with_order_by` | ~4.69 us |
| `apply_order/sqlite_desc` | ~6.88 us |

Takeaway: the query-rewriting path is cheap enough that the dominant cost quickly becomes actual database I/O rather than local SQL manipulation.

#### SQLite execution benchmarks

Seeded workload: 200 users and 500 posts in an in-memory database.

| Benchmark | Result |
|---|---:|
| `schema/list_tables` | ~19.8 us |
| `suggest_distinct_values/prefix_match` | ~36.0 us |
| `suggest_distinct_values/empty_prefix` | ~66.0 us |
| `execute_page/join` | ~100 us |
| `schema/load_diagram` | ~112 us |
| `execute_page/simple_select_page0` | ~140 us |
| `execute_page/filtered` | ~140 us |
| `full_lifecycle/order_filter_execute` | ~186 us |

Scaling note: the `SELECT * FROM users` benchmark stayed roughly flat from 100 to 1000 rows because the runtime path is page-bound and defaults to the first page.

#### PostgreSQL integration benchmarks

Seeded workload: 200 users and 500 posts in a disposable PostgreSQL container.

| Benchmark | Result |
|---|---:|
| `pg_schema/list_tables` | ~791 us |
| `pg_suggest_distinct_values/empty_prefix` | ~813 us |
| `pg_schema/get_primary_keys` | ~902 us |
| `pg_suggest_distinct_values/prefix_match` | ~936 us |
| `pg_execute_page/simple_select` | ~953 us |
| `pg_execute_page/page_1` | ~956 us |
| `pg_execute_page/with_types` | ~1.05 ms |
| `pg_full_lifecycle/order_filter_execute` | ~1.12 ms |
| `pg_schema/load_diagram` | ~3.09 ms |

Takeaway: on a local Docker-backed PostgreSQL instance, most end-to-end query paths land around the 1 ms range, while full schema-diagram loading is still in low single-digit milliseconds for the seeded dataset.

#### Redis integration benchmarks

Seeded workload: 100 string keys, one 20-field hash, and one 50-item list in a disposable Redis container.

| Benchmark | Result |
|---|---:|
| `redis_bulk/dbsize` | ~301 us |
| `redis_list/llen` | ~373 us |
| `redis_simple/get_missing` | ~377 us |
| `redis_simple/set` | ~385 us |
| `redis_list/lrange_10_items` | ~387 us |
| `redis_hash/hgetall_20_fields` | ~394 us |
| `redis_hash/hget_single` | ~396 us |
| `redis_simple/ping` | ~398 us |
| `redis_hash/hset` | ~405 us |
| `redis_simple/get_existing` | ~412 us |
| `redis_bulk/keys_star` | ~455 us |

Takeaway: Redis command execution through the core is comfortably sub-millisecond on local containerized workloads, with bulk key enumeration predictably costing more than point lookups.

### Benchmark caveat

The PostgreSQL and Redis benchmark binaries produced full benchmark results, but both currently panic during teardown because the container drop path runs outside an active Tokio reactor in the harness. The measured benchmark output still completed before the panic, so the timings above are based on emitted Criterion results, but fixing the teardown path would make the suite cleaner for CI and release reporting.

## Current limitations

This project is already useful, but it is not pretending to be feature-complete.

- Redis is supported as a query/command backend, but it does not support schema browsing, diagram generation, AST sorting/filtering, cell edits, or row deletes
- Direct edit and delete flows depend on primary keys for SQL backends
- Queries that already contain `LIMIT` keep that `LIMIT`, so built-in paging is most predictable on unrestricted queries
- Empty result sets currently do not retain column metadata in the returned result
- The macOS app does not expose Redis in its connection form yet, even though the core supports it
- CI currently validates the Rust workspace; it does not yet run an Xcode build/test job for the macOS app

## Development

### Common commands

```bash
# Build the workspace
cargo build

# Run all Rust tests
cargo test --workspace --all-targets

# Run ignored integration tests
cargo test --workspace -- --ignored

# Run the TUI
cargo run -p sbql-tui
```

### Release workflow

```bash
make version-auto
make release-dry-run
make release
```

The repository uses `release-plz` for release PRs and publishing.

## Contributing

Contributions are welcome.

If you want to add a feature, the usual path is:

1. implement it in `sbql-core`
2. wire it into `sbql-tui`
3. expose it through `sbql-ffi` if the macOS app needs it
4. update the SwiftUI client as needed

See `CONTRIBUTING.md` for project structure, architecture notes, development workflow, and testing guidance.

## License

This project is licensed under the MIT License.
