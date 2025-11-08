//! Integration test to verify AUTH command encoding with special characters

use anidb_client_core::protocol::messages::{AuthCommand, Command};
use anidb_client_core::security::SecureString;

#[test]
fn test_auth_command_integration() {
    // Test that AUTH command properly encodes special characters in passwords
    let auth = AuthCommand::new(
        "testuser@example.com".to_string(),
        SecureString::from("P@ssw0rd!#&2024"),
        "anidb-client".to_string(),
        "1.0".to_string(),
    );

    let command = Command::Auth(auth);
    let encoded = command.encode().unwrap();

    // Verify the encoded output contains properly encoded values
    println!("Encoded AUTH command: {encoded}");

    // Check that special characters are NOT URL encoded (except &)
    assert!(encoded.starts_with("AUTH"));
    assert!(encoded.contains("user=testuser@example.com"));
    assert!(encoded.contains("pass=P@ssw0rd!#&amp;2024"));
    assert!(encoded.contains("client=anidb-client"));
    assert!(encoded.contains("clientver=1.0"));
    assert!(encoded.contains("protover=3"));
    assert!(encoded.contains("enc=utf8"));

    // Verify & is encoded but other special characters are not
    // We need to find the password field more carefully since it contains "&amp;"
    let password_start = encoded.find("pass=").unwrap() + 5;
    let rest = &encoded[password_start..];
    // Find the next field separator (& not followed by amp;)
    let mut password_end = rest.len();
    let mut i = 0;
    while i < rest.len() {
        if rest[i..].starts_with("&") && !rest[i..].starts_with("&amp;") {
            password_end = i;
            break;
        }
        i += 1;
    }
    let password_value = &rest[..password_end];

    // These characters SHOULD appear in the password (not URL encoded)
    assert!(password_value.contains('@'));
    assert!(password_value.contains('!'));
    assert!(password_value.contains('#'));
    assert!(password_value.contains("&amp;")); // & should be encoded as &amp;
}

#[test]
fn test_complex_auth_encoding() {
    // Test with a very complex password containing many special characters
    let auth = AuthCommand::new(
        "user.name+tag@example.com".to_string(),
        SecureString::from("C0mpl3x!P@ss#w0rd$with%special^chars&more*symbols()"),
        "test client/v2".to_string(),
        "2.0.1".to_string(),
    );

    let command = Command::Auth(auth);
    let encoded = command.encode().unwrap();

    println!("Complex encoded AUTH command: {encoded}");

    // Verify special characters are NOT URL encoded (except &)
    assert!(encoded.contains("user=user.name+tag@example.com"));
    assert!(encoded.contains("pass=C0mpl3x!P@ss#w0rd$with%special^chars&amp;more*symbols()"));
    assert!(encoded.contains("client=test client/v2"));

    // Find the password field more carefully since it contains "&amp;"
    let password_start = encoded.find("pass=").unwrap() + 5;
    let rest = &encoded[password_start..];
    // Find the next field separator (& not followed by amp;)
    let mut password_end = rest.len();
    let mut i = 0;
    while i < rest.len() {
        if rest[i..].starts_with("&") && !rest[i..].starts_with("&amp;") {
            password_end = i;
            break;
        }
        i += 1;
    }
    let password_value = &rest[..password_end];

    // These characters SHOULD appear in the password
    assert!(password_value.contains('@'));
    assert!(password_value.contains('!'));
    assert!(password_value.contains('#'));
    assert!(password_value.contains('$'));
    assert!(password_value.contains('^'));
    assert!(password_value.contains('*'));
    assert!(password_value.contains('('));
    assert!(password_value.contains(')'));
    assert!(password_value.contains('%'));
    // & should be encoded as &amp;
    assert!(password_value.contains("&amp;more"));
}
