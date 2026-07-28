# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/Osmait/sbql/compare/sbql-core-v0.1.2...sbql-core-v0.2.0) - 2026-07-28

### Changed

- **[breaking]** `CoreEvent::Error` now carries a `CoreError { kind, severity, message, detail }`
  instead of a `String`. Clients can branch on `ErrorKind`, tell a warning from a
  failure, and read the cause chain, none of which was possible before
  ([#8](https://github.com/Osmait/sbql/pull/8))
- `Core::new` no longer treats an unreadable `connections.toml` as an empty one;
  the reason travels to clients via the new `Core::startup_events`
  ([#8](https://github.com/Osmait/sbql/pull/8))

### Fixed

- Saving a connection whose password the keyring refuses is reported as a
  warning rather than a failure — the connection *is* saved
  ([#8](https://github.com/Osmait/sbql/pull/8))
- Streaming export no longer `unwrap`s the column list
  ([#8](https://github.com/Osmait/sbql/pull/8))

## [0.1.2](https://github.com/Osmait/sbql/compare/sbql-core-v0.1.1...sbql-core-v0.1.2) - 2026-03-07

### Added

- *(diagram)* comprehensive visual improvements and navigation ([#5](https://github.com/Osmait/sbql/pull/5))

## [0.1.0](https://github.com/Osmait/sbql/releases/tag/sbql-core-v0.1.0) - 2026-03-06

### Other

- Add filter autocomplete pipeline and robust text-cast filtering
- Improve result decoding and adopt global/panel navigation flow
- Initial sbql MVP: core engine, TUI workflows, and diagram UX
