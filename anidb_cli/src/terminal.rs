//! Terminal detection and capability utilities

use is_terminal::IsTerminal;
use std::env;
use std::io::{stderr, stdout};

/// Check if stdout is connected to an interactive terminal
pub fn is_interactive() -> bool {
    // Check if stdout is a terminal
    if !stdout().is_terminal() {
        return false;
    }

    // Check for CI environments that might have TTY but shouldn't be interactive
    if is_ci_environment() {
        return false;
    }

    // Check for non-interactive shell indicators
    if env::var("DEBIAN_FRONTEND").unwrap_or_default() == "noninteractive" {
        return false;
    }

    true
}

/// Check if the terminal supports ANSI escape codes for colors and progress bars
pub fn supports_ansi() -> bool {
    // If not interactive, no ANSI support
    if !is_interactive() {
        return false;
    }

    // Check TERM environment variable
    let term = env::var("TERM").unwrap_or_default();
    if term == "dumb" || term.is_empty() {
        return false;
    }

    // Windows Terminal, ConEmu, and modern Windows consoles support ANSI
    #[cfg(windows)]
    {
        // Check for Windows Terminal
        if env::var("WT_SESSION").is_ok() {
            return true;
        }
        // Check for ConEmu
        if env::var("ConEmuANSI").unwrap_or_default() == "ON" {
            return true;
        }
        // Check Windows version for native ANSI support (Windows 10+)
        // For now, assume modern Windows supports it
        return true;
    }

    // Unix-like systems generally support ANSI unless TERM=dumb
    #[cfg(unix)]
    {
        true
    }
}

/// Check if stderr is connected to a terminal (for progress display)
pub fn stderr_is_terminal() -> bool {
    stderr().is_terminal()
}

/// Detect if running in a CI environment
fn is_ci_environment() -> bool {
    // Check common CI environment variables
    let ci_vars = [
        "CI",
        "CONTINUOUS_INTEGRATION",
        "JENKINS_URL",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "TRAVIS",
        "CIRCLECI",
        "BUILDKITE",
        "DRONE",
        "TEAMCITY_VERSION",
        "TF_BUILD", // Azure DevOps
    ];

    ci_vars.iter().any(|var| env::var(var).is_ok())
}

/// Determine if progress bars should be shown by default
pub fn should_show_progress_by_default() -> bool {
    // Progress bars should only be shown when:
    // 1. Connected to an interactive terminal
    // 2. Stderr is a terminal (progress goes to stderr)
    // 3. Terminal supports ANSI codes
    is_interactive() && stderr_is_terminal() && supports_ansi()
}

/// Check if the terminal supports hyperlinks (OSC 8 escape sequences)
#[allow(dead_code)]
pub fn supports_hyperlinks() -> bool {
    // Check if we support ANSI first
    if !supports_ansi() {
        return false;
    }

    // Check for terminals known to support hyperlinks through environment variables
    if let Ok(term) = env::var("TERM") {
        // Popular terminals with hyperlink support
        if term.contains("xterm") || term.contains("screen") || term.contains("tmux") {
            return true;
        }
    }

    // Check for terminal program environment variables
    if env::var("WEZTERM_EXECUTABLE").is_ok() {
        return true; // WezTerm supports hyperlinks
    }

    if env::var("KITTY_WINDOW_ID").is_ok() {
        return true; // Kitty supports hyperlinks
    }

    if env::var("ALACRITTY_SOCKET").is_ok() || env::var("ALACRITTY_LOG").is_ok() {
        return true; // Alacritty supports hyperlinks
    }

    // Windows Terminal supports hyperlinks
    #[cfg(windows)]
    {
        if env::var("WT_SESSION").is_ok() {
            return true;
        }
    }

    // iTerm2 supports hyperlinks
    if env::var("TERM_PROGRAM").unwrap_or_default() == "iTerm.app" {
        return true;
    }

    // VS Code integrated terminal supports hyperlinks
    if env::var("TERM_PROGRAM").unwrap_or_default() == "vscode" {
        return true;
    }

    // For other terminals, assume basic ANSI support means hyperlink support
    // This is a reasonable assumption for modern terminals
    true
}

/// Create an ANSI terminal hyperlink
///
/// Format: `\x1b]8;;{url}\x1b\\{display_text}\x1b]8;;\x1b\\`
/// If hyperlinks aren't supported, returns just the display text
#[allow(dead_code)]
pub fn hyperlink(url: &str, display_text: &str) -> String {
    if supports_hyperlinks() {
        format!("\x1b]8;;{url}\x1b\\{display_text}\x1b]8;;\x1b\\")
    } else {
        display_text.to_string()
    }
}

/// Create a hyperlink with the URL shown in parentheses as fallback
///
/// If hyperlinks are supported: `display_text` (clickable)
/// If not supported: `display_text (url)`
#[allow(dead_code)]
pub fn hyperlink_with_fallback(url: &str, display_text: &str) -> String {
    if supports_hyperlinks() {
        hyperlink(url, display_text)
    } else {
        format!("{display_text} ({url})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ci_detection() {
        // This test might pass or fail depending on the environment
        // Just ensure the function doesn't panic
        let _ = is_ci_environment();
    }

    #[test]
    fn test_terminal_detection() {
        // These might return different values in different environments
        // Just ensure they don't panic
        let _ = is_interactive();
        let _ = supports_ansi();
        let _ = stderr_is_terminal();
        let _ = should_show_progress_by_default();
    }

    #[test]
    fn test_hyperlink_functions() {
        // Just ensure the functions work and don't panic
        let _ = supports_hyperlinks();

        let url = "https://example.com";
        let text = "Example";

        let link = hyperlink(url, text);
        assert!(!link.is_empty());

        let link_with_fallback = hyperlink_with_fallback(url, text);
        assert!(!link_with_fallback.is_empty());
        assert!(link_with_fallback.contains(text));
    }

    #[test]
    fn test_hyperlink_formatting() {
        // Test hyperlink format when we know it's supported
        // We can't easily mock environment variables in tests, so we test the format directly
        let url = "https://anidb.net/file/123";
        let text = "File 123";

        // Test the format when hyperlinks are theoretically supported
        let _expected_hyperlink = format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\");

        // Test that our hyperlink function produces the right format
        // (this will depend on the current environment)
        let result = hyperlink(url, text);

        // At minimum, it should contain the display text
        assert!(result.contains(text));
    }
}
