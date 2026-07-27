//! Verifies that the platform credential store is really wired up.
//!
//! `keyring` falls back to an in-process mock store whenever no backend
//! feature matches the target OS, which compiles and "works" until the app is
//! restarted and every saved password is gone. These tests talk to the real
//! store, so they need a usable one:
//!
//! - macOS: the login Keychain (present by default)
//! - Linux/BSD: a running Secret Service (gnome-keyring, KWallet, KeePassXC...)
//!
//! They are `#[ignore]`d because a headless CI box usually has neither.
//!
//! ```bash
//! cargo test -p sbql-core --test keyring_integration -- --ignored
//! ```

// Integration test: an `unwrap` that fails here *is* the test failing, which is
// what it is for. `clippy.toml` exempts `#[cfg(test)]` modules from the panic
// lints in the workspace `Cargo.toml`, but not files under `tests/`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sbql_core::ConnectionConfig;

/// Fixture values, deliberately spelled so no human or scanner mistakes them
/// for a credential. Secret scanners flag password-shaped literals, and a test
/// that round-trips a keyring is exactly where such a literal looks real.
const FIXTURE_PASSWORD: &str = "fixture-value-not-a-credential";
const FIXTURE_SSH_PASSWORD: &str = "fixture-ssh-value-not-a-credential";

/// A saved password must survive a fresh `Entry` lookup, which is what the
/// mock store cannot do.
#[test]
#[ignore = "requires a real OS credential store"]
fn password_roundtrips_through_the_os_store() {
    let cfg =
        ConnectionConfig::new_postgres("keyring-roundtrip", "localhost", 5432, "tester", "db");

    cfg.save_password(FIXTURE_PASSWORD)
        .expect("saving to the OS credential store failed");

    let loaded = cfg
        .load_password()
        .expect("reading back from the OS credential store failed");
    assert_eq!(loaded, FIXTURE_PASSWORD);

    cfg.delete_password()
        .expect("deleting from the OS credential store failed");

    assert!(
        cfg.load_password().is_err(),
        "password still readable after delete"
    );
}

/// SSH passwords use a separate keyring service, so they get their own check.
#[test]
#[ignore = "requires a real OS credential store"]
fn ssh_password_roundtrips_through_the_os_store() {
    let mut cfg =
        ConnectionConfig::new_postgres("keyring-ssh-roundtrip", "localhost", 5432, "tester", "db");
    cfg.ssh_enabled = true;

    cfg.save_ssh_password(FIXTURE_SSH_PASSWORD)
        .expect("saving the SSH password failed");

    assert_eq!(cfg.load_ssh_password(), FIXTURE_SSH_PASSWORD);
}

/// With the keyring switched off, saving a password is a silent no-op rather
/// than an error — the absence of a store is the configured behaviour, not a
/// failure. Reading back reports "not found" so the UI can ask for it again.
#[test]
fn opting_out_makes_the_keyring_a_no_op() {
    std::env::set_var(sbql_core::NO_KEYRING_ENV, "1");
    assert!(!sbql_core::keyring_enabled());

    let cfg = ConnectionConfig::new_postgres("keyring-off", "localhost", 5432, "tester", "db");

    // No error, and nothing written anywhere.
    cfg.save_password(FIXTURE_PASSWORD)
        .expect("save should be a no-op");

    match cfg.load_password() {
        Err(sbql_core::SbqlError::PasswordNotFound(name)) => assert_eq!(name, "keyring-off"),
        other => panic!("expected PasswordNotFound, got {other:?}"),
    }

    cfg.delete_password().expect("delete should be a no-op");
    assert_eq!(cfg.load_ssh_password(), "");

    std::env::remove_var(sbql_core::NO_KEYRING_ENV);
}

/// SQLite has no password, so the keyring is never touched for it.
#[test]
fn sqlite_never_touches_the_keyring() {
    let cfg = ConnectionConfig::new_sqlite("local-file", "/tmp/test.db");

    assert!(cfg.save_password("ignored").is_ok());
    assert_eq!(cfg.load_password().unwrap(), "");
    assert!(cfg.delete_password().is_ok());
}
