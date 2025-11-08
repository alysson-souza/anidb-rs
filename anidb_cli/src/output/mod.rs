mod formatters;
mod template_helpers;

pub use formatters::{
    CsvFormatter, JsonFormatter, JsonLinesFormatter, TemplateFormatter, TextFormatter,
};

use anidb_client_core::FileResult;
use anyhow::Result;

/// Output format enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
    JsonLines,
    Csv,
    Template,
}

impl OutputFormat {
    /// Parse output format from string
    pub fn from_string(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "json-lines" | "jsonl" => Ok(Self::JsonLines),
            "csv" => Ok(Self::Csv),
            "template" => Ok(Self::Template),
            _ => anyhow::bail!("Unknown output format: {}", s),
        }
    }
}

/// Trait for output formatters
pub trait OutputFormatter: Send + Sync {
    /// Format a single result
    fn format_single(&self, result: &FileResult) -> Result<String>;

    /// Format a batch of results
    fn format_batch(&self, results: &[FileResult]) -> Result<String> {
        // Default implementation concatenates single results
        let formatted: Result<Vec<String>> =
            results.iter().map(|r| self.format_single(r)).collect();

        Ok(formatted?.join("\n"))
    }

    /// Format for streaming output (with state tracking)
    fn format_streaming(&mut self, result: &FileResult, is_first: bool) -> Result<String> {
        // Default implementation ignores streaming state
        let _ = is_first;
        self.format_single(result)
    }

    /// Finalize streaming output
    fn finalize_streaming(&self) -> Option<String> {
        None
    }
}

/// Create a formatter based on output format
pub fn create_formatter(
    format: OutputFormat,
    use_color: bool,
    template: Option<&str>,
) -> Result<Box<dyn OutputFormatter>> {
    match format {
        OutputFormat::Text => Ok(Box::new(TextFormatter::new(use_color))),
        OutputFormat::Json => Ok(Box::new(JsonFormatter::new(true))),
        OutputFormat::JsonLines => Ok(Box::new(JsonLinesFormatter::new())),
        OutputFormat::Csv => Ok(Box::new(CsvFormatter::new())),
        OutputFormat::Template => {
            let template_str = template
                .ok_or_else(|| anyhow::anyhow!("Template format requires --template argument"))?;
            Ok(Box::new(TemplateFormatter::new(template_str)?))
        }
    }
}
