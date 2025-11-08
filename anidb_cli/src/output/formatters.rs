use super::OutputFormatter;
use crate::progress::format_bytes;
use anidb_client_core::{FileResult, HashAlgorithm};
use anyhow::Result;
use colored::*;
use handlebars::Handlebars;
use serde_json::{Value, json};

/// Text formatter for human-readable output
pub struct TextFormatter {
    use_color: bool,
}

impl TextFormatter {
    pub fn new(use_color: bool) -> Self {
        Self { use_color }
    }

    fn colorize(&self, text: &str, color: fn(&str) -> ColoredString) -> String {
        if self.use_color {
            color(text).to_string()
        } else {
            text.to_string()
        }
    }
}

impl OutputFormatter for TextFormatter {
    fn format_single(&self, result: &FileResult) -> Result<String> {
        let mut output = String::new();

        // File information
        output.push_str(&format!("File: {}\n", result.file_path.display()));
        output.push_str(&format!(
            "Size: {} ({})\n",
            format_bytes(result.file_size),
            result.file_size
        ));

        // Hash results
        if !result.hashes.is_empty() {
            output.push_str("\nHashes:\n");

            // Sort hashes for consistent output
            let mut hashes: Vec<_> = result.hashes.iter().collect();
            hashes.sort_by_key(|(algo, _)| format!("{algo:?}"));

            for (algo, hash) in hashes {
                let algo_str = self.colorize(&format!("{algo:?}"), |s| s.yellow());
                let hash_str = self.colorize(hash, |s| s.cyan());
                output.push_str(&format!("  {algo_str}: {hash_str}\n"));
            }
        }

        // Processing time
        output.push_str(&format!(
            "\nProcessing time: {:.2}s\n",
            result.processing_time.as_secs_f64()
        ));

        // Anime information (if available)
        if let Some(anime_info) = &result.anime_info {
            output.push_str("\nAnime Information:\n");
            output.push_str(&format!("  Title: {}\n", anime_info.title));
            output.push_str(&format!("  Episode: {}\n", anime_info.episode_number));
        }

        Ok(output)
    }
}

/// JSON formatter for machine-readable output
pub struct JsonFormatter {
    pretty: bool,
}

impl JsonFormatter {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl OutputFormatter for JsonFormatter {
    fn format_single(&self, result: &FileResult) -> Result<String> {
        let mut json_result = json!({
            "path": result.file_path.to_string_lossy(),
            "file_size": result.file_size,
            "processing_time_ms": result.processing_time.as_millis(),
            "hashes": {},
        });

        // Add hashes
        if let Some(hashes_obj) = json_result
            .get_mut("hashes")
            .and_then(|v| v.as_object_mut())
        {
            for (algo, hash) in &result.hashes {
                hashes_obj.insert(format!("{algo:?}"), json!(hash));
            }
        }

        // Add anime info if available
        if let Some(anime_info) = &result.anime_info {
            json_result["anime_info"] = json!({
                "title": anime_info.title,
                "episode": anime_info.episode_number,
            });
        }

        if self.pretty {
            Ok(serde_json::to_string_pretty(&json_result)?)
        } else {
            Ok(serde_json::to_string(&json_result)?)
        }
    }

    fn format_batch(&self, results: &[FileResult]) -> Result<String> {
        let json_results: Vec<Value> = results
            .iter()
            .map(|r| {
                let json_str = self.format_single(r)?;
                Ok(serde_json::from_str(&json_str)?)
            })
            .collect::<Result<Vec<_>>>()?;

        if self.pretty {
            Ok(serde_json::to_string_pretty(&json_results)?)
        } else {
            Ok(serde_json::to_string(&json_results)?)
        }
    }
}

/// JSON Lines formatter for streaming output
pub struct JsonLinesFormatter;

impl Default for JsonLinesFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonLinesFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl OutputFormatter for JsonLinesFormatter {
    fn format_single(&self, result: &FileResult) -> Result<String> {
        let formatter = JsonFormatter::new(false);
        formatter.format_single(result)
    }

    fn format_batch(&self, results: &[FileResult]) -> Result<String> {
        let lines: Result<Vec<String>> = results.iter().map(|r| self.format_single(r)).collect();

        Ok(lines?.join("\n"))
    }
}

/// CSV formatter for tabular output
pub struct CsvFormatter {
    writer_initialized: bool,
}

impl Default for CsvFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl CsvFormatter {
    pub fn new() -> Self {
        Self {
            writer_initialized: false,
        }
    }

    fn get_headers() -> Vec<&'static str> {
        vec![
            "path",
            "size",
            "ed2k",
            "crc32",
            "md5",
            "sha1",
            "tth",
            "processing_time_ms",
            "anime_title",
            "anime_episode",
        ]
    }

    fn result_to_record(result: &FileResult) -> Vec<String> {
        let mut record = vec![
            result.file_path.to_string_lossy().to_string(),
            result.file_size.to_string(),
        ];

        // Add hashes in consistent order
        for algo in &[
            HashAlgorithm::ED2K,
            HashAlgorithm::CRC32,
            HashAlgorithm::MD5,
            HashAlgorithm::SHA1,
            HashAlgorithm::TTH,
        ] {
            record.push(result.hashes.get(algo).cloned().unwrap_or_default());
        }

        record.push(result.processing_time.as_millis().to_string());

        // Add anime info
        if let Some(anime_info) = &result.anime_info {
            record.push(anime_info.title.clone());
            record.push(anime_info.episode_number.to_string());
        } else {
            record.push(String::new());
            record.push(String::new());
        }

        record
    }
}

impl OutputFormatter for CsvFormatter {
    fn format_single(&self, result: &FileResult) -> Result<String> {
        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.write_record(Self::get_headers())?;
        wtr.write_record(Self::result_to_record(result))?;

        let data = wtr.into_inner()?;
        Ok(String::from_utf8(data)?)
    }

    fn format_batch(&self, results: &[FileResult]) -> Result<String> {
        let mut wtr = csv::Writer::from_writer(vec![]);
        wtr.write_record(Self::get_headers())?;

        for result in results {
            wtr.write_record(Self::result_to_record(result))?;
        }

        let data = wtr.into_inner()?;
        Ok(String::from_utf8(data)?)
    }

    fn format_streaming(&mut self, result: &FileResult, is_first: bool) -> Result<String> {
        let mut output = Vec::new();

        if is_first && !self.writer_initialized {
            // Write headers on first call
            {
                let mut wtr = csv::Writer::from_writer(&mut output);
                wtr.write_record(Self::get_headers())?;
                wtr.flush()?;
            }
            self.writer_initialized = true;
        }

        // Write the record
        {
            let mut wtr = csv::Writer::from_writer(&mut output);
            wtr.write_record(Self::result_to_record(result))?;
            wtr.flush()?;
        }

        Ok(String::from_utf8(output)?)
    }
}

/// Template formatter using Handlebars
pub struct TemplateFormatter {
    handlebars: Handlebars<'static>,
    template_name: String,
}

impl TemplateFormatter {
    pub fn new(template: &str) -> Result<Self> {
        let mut handlebars = Handlebars::new();

        // Register helpers
        super::template_helpers::register_helpers(&mut handlebars);

        // Register the template
        let template_name = "user_template";
        handlebars.register_template_string(template_name, template)?;

        Ok(Self {
            handlebars,
            template_name: template_name.to_string(),
        })
    }
}

impl OutputFormatter for TemplateFormatter {
    fn format_single(&self, result: &FileResult) -> Result<String> {
        // Convert FileResult to JSON for template rendering
        let mut data = json!({
            "path": result.file_path.to_string_lossy(),
            "file_size": result.file_size,
            "processing_time_ms": result.processing_time.as_millis(),
            "hashes": {},
        });

        // Add hashes
        if let Some(hashes_obj) = data.get_mut("hashes").and_then(|v| v.as_object_mut()) {
            for (algo, hash) in &result.hashes {
                hashes_obj.insert(format!("{algo:?}"), json!(hash));
            }
        }

        // Add anime info if available
        if let Some(anime_info) = &result.anime_info {
            data["anime_info"] = json!({
                "title": anime_info.title,
                "episode": anime_info.episode_number,
            });
        }

        Ok(self.handlebars.render(&self.template_name, &data)?)
    }
}
