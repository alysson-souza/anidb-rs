//! Tests for AniDB protocol parameter encoding
//!
//! Ensures that all parameter values are properly encoded according to
//! AniDB protocol requirements.

use anidb_client_core::protocol::messages::{AniDBCommand, AuthCommand, encode_value};

#[test]
fn test_encode_value_basic_html_entities() {
    // Basic HTML entity encoding
    assert_eq!(encode_value("simple"), "simple");
    assert_eq!(encode_value("with&ampersand"), "with&amp;ampersand");
    assert_eq!(encode_value("multiple&&&"), "multiple&amp;&amp;&amp;");
}

#[test]
fn test_encode_value_special_characters() {
    // Special characters are NOT URL encoded (except &)
    assert_eq!(encode_value("user@example.com"), "user@example.com");
    assert_eq!(encode_value("pass!word"), "pass!word");
    assert_eq!(encode_value("test#tag"), "test#tag");
    assert_eq!(encode_value("value=test"), "value=test");
    assert_eq!(encode_value("space test"), "space test");
    assert_eq!(encode_value("test?query"), "test?query");
    assert_eq!(encode_value("path/to/file"), "path/to/file");
    assert_eq!(encode_value("test:value"), "test:value");
    assert_eq!(encode_value("test;value"), "test;value");
    assert_eq!(encode_value("test<value>"), "test<value>");
    assert_eq!(encode_value("test\"value\""), "test\"value\"");
    assert_eq!(encode_value("test'value'"), "test'value'");
    assert_eq!(encode_value("test\\value"), "test\\value");
    assert_eq!(encode_value("test|value"), "test|value");
    assert_eq!(encode_value("test[value]"), "test[value]");
    assert_eq!(encode_value("test{value}"), "test{value}");
    assert_eq!(encode_value("test+value"), "test+value");
    assert_eq!(encode_value("test%value"), "test%value");
}

#[test]
fn test_encode_value_complex_password() {
    // Complex password with multiple special characters
    let password = "P@ssw0rd!#2024";
    let encoded = encode_value(password);
    assert_eq!(encoded, "P@ssw0rd!#2024"); // No URL encoding

    // Another complex example with ampersand
    let complex = "Test&User@2024!";
    let encoded = encode_value(complex);
    assert_eq!(encoded, "Test&amp;User@2024!"); // Only & is encoded
}

#[test]
fn test_encode_value_newlines() {
    // Newlines should be encoded as <br />
    assert_eq!(encode_value("line1\nline2"), "line1<br />line2");
    assert_eq!(encode_value("line1\r\nline2"), "line1<br />line2");
}

#[test]
fn test_encode_value_mixed_encoding() {
    // Mix of HTML entities and newline encoding (no URL encoding)
    let mixed = "user&pass@test.com\nline2";
    let encoded = encode_value(mixed);
    assert_eq!(encoded, "user&amp;pass@test.com<br />line2");
}

#[test]
fn test_auth_command_encoding() {
    use anidb_client_core::security::SecureString;

    // Create AUTH command with special characters
    let auth = AuthCommand::new(
        "user@example.com".to_string(),
        SecureString::from("P@ssw0rd!#2024"),
        "testclient".to_string(),
        "1.0".to_string(),
    );

    let encoded = auth.encode().unwrap();

    // Verify the encoded command contains properly encoded values (no URL encoding)
    assert!(encoded.contains("user=user@example.com"));
    assert!(encoded.contains("pass=P@ssw0rd!#2024"));
    assert!(encoded.contains("client=testclient"));
    assert!(encoded.contains("clientver=1.0"));
}

#[test]
fn test_auth_command_with_ampersand() {
    use anidb_client_core::security::SecureString;

    // Password with ampersand should use HTML entity encoding
    let auth = AuthCommand::new(
        "testuser".to_string(),
        SecureString::from("pass&word"),
        "client".to_string(),
        "1.0".to_string(),
    );

    let encoded = auth.encode().unwrap();
    assert!(encoded.contains("pass=pass&amp;word"));
}

#[test]
fn test_encode_value_unicode() {
    // Unicode characters should be preserved (UTF-8)
    assert_eq!(encode_value("日本語"), "日本語");
    assert_eq!(encode_value("émoji 😀"), "émoji 😀"); // Space is not encoded
}

#[test]
fn test_encode_value_empty_string() {
    assert_eq!(encode_value(""), "");
}

#[test]
fn test_encode_value_only_special_chars() {
    assert_eq!(
        encode_value("@!#$%^&*()"),
        "@!#$%^&amp;*()" // Only & is encoded
    );
}
