use colored::*;
use std::error::Error as StdError;
use std::fmt;
use std::io;

/// CLI-specific error type with semantic exit codes
#[derive(Debug)]
pub struct CliError {
    /// The main error message
    message: String,

    /// Error category for exit code determination
    category: ErrorCategory,

    /// Additional context information
    context: Vec<(String, String)>,

    /// Suggestions for recovery
    pub suggestions: Vec<String>,

    /// Source error if any
    source: Option<Box<dyn StdError + Send + Sync>>,
}

/// Error categories that map to exit codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorCategory {
    Success,
    General,
    Misuse,
    Network,
    Filesystem,
}

/// Semantic exit codes for the CLI
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ExitCode {
    Success = 0,
    GeneralError = 1,
    Misuse = 2,
    NetworkError = 3,
    FilesystemError = 4,
}

/// Result type for CLI operations
pub type CliResult<T> = Result<T, CliError>;

/// Extension trait for adding context to errors
pub trait ErrorContext {
    fn with_context(self, key: &str, value: &str) -> Self;
    fn with_suggestion(self, suggestion: &str) -> Self;
    fn with_source(self, source: Box<dyn StdError + Send + Sync>) -> Self;
}

impl CliError {
    /// Create a success "error" (for consistent handling)
    pub fn success(message: &str) -> Self {
        Self {
            message: message.to_string(),
            category: ErrorCategory::Success,
            context: Vec::new(),
            suggestions: Vec::new(),
            source: None,
        }
    }

    /// Create a general error
    pub fn general(message: &str) -> Self {
        Self {
            message: message.to_string(),
            category: ErrorCategory::General,
            context: Vec::new(),
            suggestions: Vec::new(),
            source: None,
        }
    }

    /// Create a command misuse error
    pub fn misuse(message: &str) -> Self {
        let mut error = Self {
            message: message.to_string(),
            category: ErrorCategory::Misuse,
            context: Vec::new(),
            suggestions: vec!["Run 'anidb --help' for usage information".to_string()],
            source: None,
        };

        // Add specific suggestions based on the message
        if message.contains("Unknown command")
            && let Some(cmd) = message.split(':').nth(1).map(|s| s.trim())
        {
            // Simple typo detection
            let commands = ["hash", "identify", "process", "config", "auth", "sync"];
            for known_cmd in commands {
                if levenshtein_distance(cmd, known_cmd) <= 2 {
                    error
                        .suggestions
                        .insert(0, format!("Did you mean '{known_cmd}'?"));
                    break;
                }
            }
        }

        error
    }

    /// Create a network error
    pub fn network(message: &str) -> Self {
        Self {
            message: message.to_string(),
            category: ErrorCategory::Network,
            context: Vec::new(),
            suggestions: vec![
                "Check your internet connection".to_string(),
                "Verify AniDB server status at https://anidb.net".to_string(),
                "Try again later".to_string(),
            ],
            source: None,
        }
    }

    /// Create a filesystem error
    pub fn filesystem(message: &str) -> Self {
        let mut error = Self {
            message: message.to_string(),
            category: ErrorCategory::Filesystem,
            context: Vec::new(),
            suggestions: Vec::new(),
            source: None,
        };

        // Add specific suggestions based on the message
        if message.contains("not found") {
            error
                .suggestions
                .push("Check if the file or directory exists".to_string());
            error
                .suggestions
                .push("Verify you have the correct path".to_string());
        } else if message.contains("permission") || message.contains("denied") {
            error.suggestions.push("Check file permissions".to_string());
            error
                .suggestions
                .push("Try running with elevated permissions if needed".to_string());
        }

        error
    }

    /// Create an error from an IO error
    pub fn from_io_error(error: io::Error, path: &str) -> Self {
        let message = format!("IO error on '{path}': {error}");
        let mut cli_error = match error.kind() {
            io::ErrorKind::NotFound => Self::filesystem(&message),
            io::ErrorKind::PermissionDenied => Self::filesystem(&message),
            io::ErrorKind::TimedOut => Self::network(&message),
            io::ErrorKind::UnexpectedEof => Self::network(&message),
            _ => Self::general(&message),
        };

        cli_error.source = Some(Box::new(error));
        cli_error
            .context
            .push(("path".to_string(), path.to_string()));
        cli_error
    }

    /// Get the exit code for this error
    pub fn exit_code(&self) -> ExitCode {
        match self.category {
            ErrorCategory::Success => ExitCode::Success,
            ErrorCategory::General => ExitCode::GeneralError,
            ErrorCategory::Misuse => ExitCode::Misuse,
            ErrorCategory::Network => ExitCode::NetworkError,
            ErrorCategory::Filesystem => ExitCode::FilesystemError,
        }
    }

    /// Format the error for user display
    pub fn format_for_user(&self, debug: bool) -> String {
        let mut output = String::new();

        // Main error message
        let prefix = match self.category {
            ErrorCategory::Success => "Success".green(),
            ErrorCategory::General => "Error".red(),
            ErrorCategory::Misuse => "Usage Error".yellow(),
            ErrorCategory::Network => "Network Error".red(),
            ErrorCategory::Filesystem => "File Error".red(),
        };

        output.push_str(&format!("{}: {}\n", prefix, self.message));

        // Context information
        if !self.context.is_empty() {
            output.push_str("\nContext:\n");
            for (key, value) in &self.context {
                output.push_str(&format!("  {}: {}\n", key.bold(), value));
            }
        }

        // Error chain in debug mode
        if debug && let Some(source) = &self.source {
            output.push_str("\nCaused by:\n");
            let mut current: Option<&dyn StdError> = Some(source.as_ref());
            let mut level = 1;

            while let Some(err) = current {
                output.push_str(&format!("  {level}: {err}\n"));
                current = err.source();
                level += 1;
            }
        }

        // Suggestions
        if !self.suggestions.is_empty() {
            output.push_str("\nSuggestions:\n");
            for suggestion in &self.suggestions {
                output.push_str(&format!("  • {suggestion}\n"));
            }
        }

        output
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {}",
            match self.category {
                ErrorCategory::Success => "Success",
                ErrorCategory::General => "Error",
                ErrorCategory::Misuse => "Usage Error",
                ErrorCategory::Network => "Network Error",
                ErrorCategory::Filesystem => "File Error",
            },
            self.message
        )?;

        // Include context in display
        for (key, value) in &self.context {
            write!(f, " ({key}: {value})")?;
        }

        Ok(())
    }
}

impl StdError for CliError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn StdError + 'static))
    }
}

impl ErrorContext for CliError {
    fn with_context(mut self, key: &str, value: &str) -> Self {
        self.context.push((key.to_string(), value.to_string()));
        self
    }

    fn with_suggestion(mut self, suggestion: &str) -> Self {
        self.suggestions.push(suggestion.to_string());
        self
    }

    fn with_source(mut self, source: Box<dyn StdError + Send + Sync>) -> Self {
        self.source = Some(source);
        self
    }
}

/// Convert anyhow errors to CLI errors
impl From<anyhow::Error> for CliError {
    fn from(error: anyhow::Error) -> Self {
        Self::general(&error.to_string())
    }
}

/// Simple Levenshtein distance for command suggestions
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let s1_chars: Vec<char> = s1.chars().collect();
    let s2_chars: Vec<char> = s2.chars().collect();
    let len1 = s1_chars.len();
    let len2 = s2_chars.len();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for (i, row) in matrix.iter_mut().enumerate().take(len1 + 1) {
        row[0] = i;
    }
    for (j, cell) in matrix[0].iter_mut().enumerate().take(len2 + 1) {
        *cell = j;
    }

    for (i, c1) in s1_chars.iter().enumerate() {
        let i1 = i + 1;
        for (j, c2) in s2_chars.iter().enumerate() {
            let j1 = j + 1;
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i1][j1] = std::cmp::min(
                std::cmp::min(matrix[i][j1] + 1, matrix[i1][j] + 1),
                matrix[i][j] + cost,
            );
        }
    }

    matrix[len1][len2]
}
