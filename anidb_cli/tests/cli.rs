use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_version() {
    let mut cmd = Command::cargo_bin("anidb").unwrap();
    cmd.arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_hash_sha1() {
    // Create a temporary test file
    let temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), b"test content").unwrap();

    let mut cmd = Command::cargo_bin("anidb").unwrap();
    cmd.arg("hash")
        .arg(temp_file.path())
        .arg("--algorithm")
        .arg("sha1")
        .assert()
        .success()
        .stdout(predicate::str::contains("SHA1:"));
}

#[test]
fn test_hash_tth() {
    // Create a temporary test file
    let temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), b"test content").unwrap();

    let mut cmd = Command::cargo_bin("anidb").unwrap();
    cmd.arg("hash")
        .arg(temp_file.path())
        .arg("--algorithm")
        .arg("tth")
        .assert()
        .success()
        .stdout(predicate::str::contains("TTH:"));
}

#[test]
fn test_hash_all_includes_sha1_and_tth() {
    // Create a temporary test file
    let temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), b"test content").unwrap();

    let mut cmd = Command::cargo_bin("anidb").unwrap();
    cmd.arg("hash")
        .arg(temp_file.path())
        .arg("--algorithm")
        .arg("all")
        .assert()
        .success()
        .stdout(predicate::str::contains("ED2K:"))
        .stdout(predicate::str::contains("CRC32:"))
        .stdout(predicate::str::contains("SHA1:"))
        .stdout(predicate::str::contains("TTH:"));
}
