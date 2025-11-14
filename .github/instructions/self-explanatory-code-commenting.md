---
description: 'Guidelines for GitHub Copilot to write comments to achieve self-explanatory Rust code with fewer comments. Examples are in Rust but principles apply to any language that has comments.'
applyTo: '**'
---

# Self-explanatory Code Commenting Instructions

## Core Principle
**Write code that speaks for itself. Comment only when necessary to explain WHY, not WHAT.**
We do not need comments most of the time.

## Commenting Guidelines

### ❌ AVOID These Comment Types

**Obvious Comments**
```rust
// Bad: States the obvious
let mut counter = 0; // Initialize counter to zero
counter += 1;        // Increment counter by one
```

**Redundant Comments**
```rust
// Bad: Comment repeats the code
struct User { name: String }

fn user_name(user: &User) -> &str {
    // Return the user's name
    &user.name
}
```

**Outdated Comments**
```rust
// Bad: Comment doesn't match the code
// Calculate tax at 5% rate
let tax = price * 0.08; // Actually 8%
```

### ✅ WRITE These Comment Types

**Complex Business Logic**
```rust
// Good: Explains WHY this specific calculation
// Apply progressive tax brackets: 10% up to 10k, 20% above
let tax = calculate_progressive_tax(income, &[0.10, 0.20], &[10_000]);
```

**Non-obvious Algorithms**
```rust
// Good: Explains the algorithm choice
// Using Floyd–Warshall for all-pairs shortest paths
// because we need distances between all nodes
for k in 0..n {
    for i in 0..n {
        for j in 0..n {
            // ... implementation
        }
    }
}
```

**Regex Patterns**
```rust
// Good: Explains what the regex matches
// Match email format: username@domain.extension
use regex::Regex;
let email_pattern = Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap();
```

**API Constraints or Gotchas**
```rust
// Good: Explains external constraint
// GitHub API rate limit: 5000 requests/hour for authenticated users
rate_limiter.wait().await?;
let response = reqwest::get(github_api_url).await?;
```

## Decision Framework

Before writing a comment, ask:
1. **Is the code self-explanatory?** → No comment needed
2. **Would a better variable/function name eliminate the need?** → Refactor instead
3. **Does this explain WHY, not WHAT?** → Good comment
4. **Will this help future maintainers?** → Good comment

## Special Cases for Comments

### Public APIs
```rust
/// Calculate compound interest using the standard formula.
///
/// # Arguments
/// * `principal` - Initial amount invested.
/// * `rate` - Annual interest rate as decimal (e.g. 0.05 for 5%).
/// * `time_years` - Time period in years.
/// * `compound_frequency` - Times per year interest compounds (default: 1).
///
/// # Returns
/// Final amount after compound interest.
///
/// # Examples
/// ```
/// let amount = calculate_compound_interest(1000.0, 0.05, 10.0, 1);
/// assert!((amount - 1628.894626777441).abs() < 0.01);
/// ```
pub fn calculate_compound_interest(
    principal: f64,
    rate: f64,
    time_years: f64,
    compound_frequency: u32,
) -> f64 {
    let n = compound_frequency as f64;
    principal * (1.0 + rate / n).powf(n * time_years)
}
```

### Configuration and Constants
```rust
// Good: Explains the source or reasoning
const MAX_RETRIES: u32 = 3;      // Based on observed network reliability
const API_TIMEOUT_MS: u64 = 5_000; // AWS Lambda timeout is 15s; leave buffer
```

### Annotations
```rust
// TODO: Replace with proper user authentication after security review
// FIXME: Memory leak in production - investigate connection pooling
// HACK: Workaround for bug in library v2.1.0 - remove after upgrade
// NOTE: This implementation assumes UTC timezone for all calculations
// WARNING: This function mutates the input data structure
// PERF: Consider caching this result if called frequently in a hot path
// SECURITY: Validate input before constructing SQL queries
// BUG: Edge case failure when slice is empty - needs investigation
// REFACTOR: Extract this logic into a utility module for reuse
// DEPRECATED: Use new_api_function() instead - this will be removed in v3.0
```

## Anti-Patterns to Avoid

### Dead Code Comments
```rust
// Bad: Don't comment out code
// fn old_function() {}
fn new_function() {}
```

### Changelog Comments
```rust
// Bad: Don't maintain history in comments
// Modified by John on 2023-01-15
// Fixed bug reported by Sarah on 2023-02-03
fn process_data() {
    // ... implementation
}
```

### Divider Comments
```rust
// Bad: Don't use decorative comments
//=====================================
// UTILITY FUNCTIONS
//=====================================
```

## Quality Checklist

Before committing, ensure your comments:
- [ ] Explain WHY, not WHAT
- [ ] Are grammatically correct and clear
- [ ] Will remain accurate as code evolves
- [ ] Add genuine value to code understanding
- [ ] Are placed appropriately (above the code they describe)
- [ ] Use proper spelling and professional language

## Summary

Remember: **The best comment is the one you don't need to write because the code is self-documenting.**