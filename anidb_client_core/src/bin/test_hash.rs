use anidb_client_core::hashing::{HashAlgorithm, HashCalculator};

fn main() {
    let calculator = HashCalculator::new();

    println!("Testing ED2K (MD4) hashes:");
    let test_cases: Vec<(&[u8], &str)> = vec![
        (b"", "31d6cfe0d16ae931b73c59d7e0c089c0"),
        (b"a", "bde52cb31de33e46245e05fbdbd6fb24"),
        (b"test content", "098f6bcd4621d373cade4e832627b4f6"),
    ];

    for (input, expected) in test_cases {
        let result = calculator
            .calculate_bytes(HashAlgorithm::ED2K, input)
            .unwrap();
        println!(
            "Input: {:?}, Expected: {}, Got: {}, Match: {}",
            std::str::from_utf8(input).unwrap_or("<binary>"),
            expected,
            result.hash,
            result.hash == expected
        );
    }

    println!("\nTesting CRC32 hashes:");
    let test_cases: Vec<(&[u8], &str)> = vec![
        (b"", "00000000"),
        (b"a", "e8b7be43"),
        (b"test content", "d87f7e0c"),
    ];

    for (input, expected) in test_cases {
        let result = calculator
            .calculate_bytes(HashAlgorithm::CRC32, input)
            .unwrap();
        println!(
            "Input: {:?}, Expected: {}, Got: {}, Match: {}",
            std::str::from_utf8(input).unwrap_or("<binary>"),
            expected,
            result.hash,
            result.hash == expected
        );
    }
}
