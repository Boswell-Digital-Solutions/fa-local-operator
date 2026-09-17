use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::adapters::exports::{ForensicEventExportAdapter, ForensicExportResult};
use crate::domain::forensics::ValidatedForensicEvent;

/// Append-only local JSONL forensic export sink: one compact JSON object per
/// line, one line per event. This is FA Local's own minimal, locally
/// auditable forensic record (`FORENSICS.md`) — not the durable canonical
/// persistence layer, which stays DataForge Local's responsibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlForensicExportAdapterConfig {
    pub export_file_path: PathBuf,
}

impl JsonlForensicExportAdapterConfig {
    pub fn new(export_file_path: PathBuf) -> Self {
        Self { export_file_path }
    }
}

#[derive(Debug, Clone)]
pub struct JsonlForensicExportAdapter {
    config: JsonlForensicExportAdapterConfig,
}

impl JsonlForensicExportAdapter {
    pub fn new(config: JsonlForensicExportAdapterConfig) -> Self {
        Self { config }
    }

    pub fn export_file_path(&self) -> &Path {
        &self.config.export_file_path
    }
}

impl ForensicEventExportAdapter for JsonlForensicExportAdapter {
    fn adapter_id(&self) -> &'static str {
        "jsonl-forensic-export"
    }

    fn export_event(&self, event: &ValidatedForensicEvent) -> ForensicExportResult {
        let line = match serde_json::to_string(&event.event) {
            Ok(line) => line,
            Err(_) => {
                return ForensicExportResult::Unsupported {
                    summary: "forensic event could not be serialized for jsonl export".to_owned(),
                };
            }
        };

        let mut file = match OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.export_file_path())
        {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                return ForensicExportResult::DependencyUnavailable {
                    summary: "jsonl forensic export directory is unavailable".to_owned(),
                };
            }
            Err(_) => {
                return ForensicExportResult::DependencyUnavailable {
                    summary: "jsonl forensic export sink is unavailable".to_owned(),
                };
            }
        };

        if writeln!(file, "{line}").is_err() {
            return ForensicExportResult::DependencyUnavailable {
                summary: "jsonl forensic export write failed".to_owned(),
            };
        }

        ForensicExportResult::Exported {
            export_reference: event.event.forensic_event_id.to_string(),
        }
    }
}
