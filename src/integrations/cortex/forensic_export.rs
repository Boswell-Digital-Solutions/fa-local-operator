//! Export sinks for [`GnatDispatchForensicEvent`](crate::integrations::cortex::GnatDispatchForensicEvent).
//!
//! Mirrors the JSONL/SQLite forensic export sinks `adapters::exports`
//! already provides for FA Local's own `ForensicEvent` family -- a
//! separate sink for a separate contract, the same reason
//! `forensics.rs` gives that event family its own schema instead of
//! reusing `forensic-event.schema.json`. Closes the disclosed gap noted
//! throughout `GnatDispatchPipelineService` and `ROADMAP.md`: recording
//! was in-memory only, returned to the caller and never persisted.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::adapters::exports::ForensicExportResult;
use crate::errors::FaLocalResult;
use crate::integrations::cortex::{GnatDispatchForensicEvent, ValidatedGnatDispatchForensicEvent};

pub trait GnatForensicEventExportAdapter {
    fn adapter_id(&self) -> &'static str;

    fn export_event(&self, event: &ValidatedGnatDispatchForensicEvent) -> ForensicExportResult;
}

/// Append-only local JSONL export sink for Gnat dispatch forensic events:
/// one compact JSON object per line, one line per event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlGnatForensicExportAdapterConfig {
    pub export_file_path: PathBuf,
}

impl JsonlGnatForensicExportAdapterConfig {
    pub fn new(export_file_path: PathBuf) -> Self {
        Self { export_file_path }
    }
}

#[derive(Debug, Clone)]
pub struct JsonlGnatForensicExportAdapter {
    config: JsonlGnatForensicExportAdapterConfig,
}

impl JsonlGnatForensicExportAdapter {
    pub fn new(config: JsonlGnatForensicExportAdapterConfig) -> Self {
        Self { config }
    }

    pub fn export_file_path(&self) -> &Path {
        &self.config.export_file_path
    }
}

impl GnatForensicEventExportAdapter for JsonlGnatForensicExportAdapter {
    fn adapter_id(&self) -> &'static str {
        "jsonl-gnat-forensic-export"
    }

    fn export_event(&self, event: &ValidatedGnatDispatchForensicEvent) -> ForensicExportResult {
        let line = match serde_json::to_string(&event.event) {
            Ok(line) => line,
            Err(_) => {
                return ForensicExportResult::Unsupported {
                    summary:
                        "gnat dispatch forensic event could not be serialized for jsonl export"
                            .to_owned(),
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
                    summary: "jsonl gnat forensic export directory is unavailable".to_owned(),
                };
            }
            Err(_) => {
                return ForensicExportResult::DependencyUnavailable {
                    summary: "jsonl gnat forensic export sink is unavailable".to_owned(),
                };
            }
        };

        if writeln!(file, "{line}").is_err() {
            return ForensicExportResult::DependencyUnavailable {
                summary: "jsonl gnat forensic export write failed".to_owned(),
            };
        }

        ForensicExportResult::Exported {
            export_reference: event.event.forensic_event_id.to_string(),
        }
    }
}

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS gnat_dispatch_forensic_events (
    forensic_event_id TEXT PRIMARY KEY,
    correlation_id TEXT NOT NULL,
    run_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    shard_id TEXT,
    timestamp_utc TEXT NOT NULL,
    event_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_gnat_dispatch_forensic_events_run_id ON gnat_dispatch_forensic_events(run_id);
CREATE INDEX IF NOT EXISTS idx_gnat_dispatch_forensic_events_event_type ON gnat_dispatch_forensic_events(event_type);
";

/// Queryable local forensic store for Gnat dispatch forensic events, backed
/// by bundled SQLite -- the same round-trip-the-whole-event-as-JSON shape
/// `adapters::exports::sqlite_forensic_store::SqliteForensicStore` uses for
/// FA Local's own forensic-event family.
#[derive(Debug)]
pub struct SqliteGnatForensicStore {
    connection: Mutex<Connection>,
}

impl SqliteGnatForensicStore {
    pub fn open(path: &Path) -> FaLocalResult<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(CREATE_SCHEMA)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn insert(&self, event: &GnatDispatchForensicEvent) -> FaLocalResult<()> {
        let connection = self
            .connection
            .lock()
            .expect("sqlite connection mutex poisoned");
        let event_json = serde_json::to_string(event)?;

        connection.execute(
            "INSERT INTO gnat_dispatch_forensic_events
                (forensic_event_id, correlation_id, run_id, event_type, shard_id, timestamp_utc, event_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.forensic_event_id.to_string(),
                event.correlation_id.to_string(),
                event.run_id,
                enum_text(&event.event_type),
                event.shard_id,
                event.timestamp_utc.to_rfc3339(),
                event_json,
            ],
        )?;

        Ok(())
    }
}

impl GnatForensicEventExportAdapter for SqliteGnatForensicStore {
    fn adapter_id(&self) -> &'static str {
        "sqlite-gnat-forensic-store"
    }

    fn export_event(&self, event: &ValidatedGnatDispatchForensicEvent) -> ForensicExportResult {
        match self.insert(&event.event) {
            Ok(()) => ForensicExportResult::Exported {
                export_reference: event.event.forensic_event_id.to_string(),
            },
            Err(_) => ForensicExportResult::DependencyUnavailable {
                summary: "sqlite gnat forensic store write failed".to_owned(),
            },
        }
    }
}

fn enum_text<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value).expect("forensic enum values always serialize") {
        serde_json::Value::String(text) => text,
        other => {
            unreachable!("forensic enum serialization must produce a JSON string, got {other:?}")
        }
    }
}
