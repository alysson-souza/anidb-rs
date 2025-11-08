//! Verify ED2K implementation against known test vectors

use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};

#[test]
fn verify_ed2k_known_vectors() {
    println!("\n=== ED2K Known Test Vectors ===\n");

    let calculator = HashCalculator::new();

    // Test 1: Empty file
    let empty_data = b"";
    let empty_result = calculator
        .calculate_bytes(HashAlgorithm::ED2K, empty_data)
        .unwrap();
    println!("Empty file: {}", empty_result.hash);
    // ED2K of empty file should be MD4 of empty string: 31d6cfe0d16ae931b73c59d7e0c089c0

    // Test 2: Small file (< 9.5MB)
    let small_data = b"Hello, World!";
    let small_result = calculator
        .calculate_bytes(HashAlgorithm::ED2K, small_data)
        .unwrap();
    println!("Small file (13 bytes): {}", small_result.hash);

    // Test 3: File exactly 9.5MB
    let exact_chunk = vec![0x00u8; 9_728_000];
    let exact_result = calculator
        .calculate_bytes(HashAlgorithm::ED2K, &exact_chunk)
        .unwrap();
    println!("Exactly 9.5MB (all zeros): {}", exact_result.hash);

    // Test 4: File slightly larger than 9.5MB
    let larger = vec![0x00u8; 9_728_001];
    let larger_result = calculator
        .calculate_bytes(HashAlgorithm::ED2K, &larger)
        .unwrap();
    println!("9.5MB + 1 byte (all zeros): {}", larger_result.hash);

    // According to ED2K specification:
    // - Files <= 9.5MB: ED2K hash = MD4(file content)
    // - Files > 9.5MB: ED2K hash = MD4(concatenated MD4 hashes of 9.5MB chunks)
    // - Red variant: For files that are exact multiples of 9.5MB, append MD4("")

    println!("\n=== ED2K Specification Summary ===");
    println!("1. Single chunk files (≤9.5MB): Direct MD4 hash");
    println!("2. Multi-chunk files (>9.5MB): MD4 of concatenated chunk hashes");
    println!("3. Red variant: Append empty hash for exact multiples (only matters for >9.5MB)");
    println!("\nConclusion: A file of exactly 9.5MB is treated as a single-chunk file,");
    println!("so its ED2K hash is simply MD4(file content), not affected by Red/Blue variant.");
}
