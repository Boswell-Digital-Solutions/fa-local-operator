//! Export sinks for [`NeuronForgeTaskDispatchForensicEvent`](crate::integrations::neuronforge_local::NeuronForgeTaskDispatchForensicEvent).
//!
//! Mirrors `integrations::cortex::forensic_export` -- the JSONL/SQLite
//! export sinks for `GnatDispatchForensicEvent` -- for this separate
//! contract, the same reason `forensics.rs` gives that event family its own
//! schema instead of reusing `forensic-event.schema.json`.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::adapters::exports::ForensicExportResult;
use crate::errors::FaLocalResult;
use crate::integrations::neuronforge_local::{
    NeuronForgeTaskDispatchForensicEvent, ValidatedNeuronForgeTaskDispatchForensicEvent,
};

pub trait NeuronForgeForensicEventExportAdapter {
    fn adapter_id(&self) -> &'static str;

    fn export_event(
        &self,
        event: &ValidatedNeuronForgeTaskDispatchForensicEvent,
    ) -> ForensicExportResult;
}

/// Append-only local JSONL export sink for NeuronForge task-dispatch
/// forensic events: one compact JSON object per line, one line per event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonlNeuronForgeForensicExportAdapterConfig {
    pub export_file_path: PathBuf,
}

impl JsonlNeuronForgeForensicExportAdapterConfig {
    pub fn new(export_file_path: PathBuf) -> Self {
        Self { export_file_path }
    }
}

#[derive(Debug, Clone)]
pub struct JsonlNeuronForgeForensicExportAdapter {
    config: JsonlNeuronForgeForensicExportAdapterConfig,
}

impl JsonlNeuronForgeForensicExportAdapter {
    pub fn new(config: JsonlNeuronForgeForensicExportAdapterConfig) -> Self {
        Self { config }
    }

    pub fn export_file_path(&self) -> &Path {
        &self.config.export_file_path
    }
}

impl NeuronForgeForensicEventExportAdapter for JsonlNeuronForgeForensicExportAdapter {
    fn adapter_id(&self) -> &'static str {
        "jsonl-neuronforge-forensic-export"
    }

    fn export_event(
        &self,
        event: &ValidatedNeuronForgeTaskDispatchForensicEvent,
    ) -> ForensicExportResult {
        let line = match serde_json::to_string(&event.event) {
            Ok(line) => line,
            Err(_) => {
                return ForensicExportResult::Unsupported {
                    summary: "neuronforge task dispatch forensic event could not be serialized for jsonl export"
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
                    summary: "jsonl neuronforge forensic export directory is unavailable"
                        .to_owned(),
                };
            }
            Err(_) => {
                return ForensicExportResult::DependencyUnavailable {
                    summary: "jsonl neuronforge forensic export sink is unavailable".to_owned(),
                };
            }
        };

        if writeln!(file, "{line}").is_err() {
            return ForensicExportResult::DependencyUnavailable {
                summary: "jsonl neuronforge forensic export write failed".to_owned(),
            };
        }

        ForensicExportResult::Exported {
            export_reference: event.event.forensic_event_id.to_string(),
        }
    }
}

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS neuronforge_task_dispatch_forensic_events (
    forensic_event_id TEXT PRIMARY KEY,
    dispatch_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    outcome TEXT NOT NULL,
    timestamp_utc TEXT NOT NULL,
    event_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_neuronforge_task_dispatch_forensic_events_dispatch_id ON neuronforge_task_dispatch_forensic_events(dispatch_id);
CREATE INDEX IF NOT EXISTS idx_neuronforge_task_dispatch_forensic_events_outcome ON neuronforge_task_dispatch_forensic_events(outcome);
";

/// Queryable local forensic store for NeuronForge task-dispatch forensic
/// events, backed by bundled SQLite -- the same round-trip-the-whole-event-
/// as-JSON shape `integrations::cortex::forensic_export::SqliteGnatForensicStore`
/// uses for its own event family.
#[derive(Debug)]
pub struct SqliteNeuronForgeForensicStore {
    connection: Mutex<Connection>,
}

impl SqliteNeuronForgeForensicStore {
    pub fn open(path: &Path) -> FaLocalResult<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(CREATE_SCHEMA)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn insert(&self, event: &NeuronForgeTaskDispatchForensicEvent) -> FaLocalResult<()> {
        let connection = self
            .connection
            .lock()
            .expect("sqlite connection mutex poisoned");
        let event_json = serde_json::to_string(event)?;

        connection.execute(
            "INSERT INTO neuronforge_task_dispatch_forensic_events
                (forensic_event_id, dispatch_id, request_id, task_id, outcome, timestamp_utc, event_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.forensic_event_id.to_string(),
                event.dispatch_id,
                event.request_id,
                event.task_id,
                enum_text(&event.outcome),
                event.timestamp_utc.to_rfc3339(),
                event_json,
            ],
        )?;

        Ok(())
    }
}

impl NeuronForgeForensicEventExportAdapter for SqliteNeuronForgeForensicStore {
    fn adapter_id(&self) -> &'static str {
        "sqlite-neuronforge-forensic-store"
    }

    fn export_event(
        &self,
        event: &ValidatedNeuronForgeTaskDispatchForensicEvent,
    ) -> ForensicExportResult {
        match self.insert(&event.event) {
            Ok(()) => ForensicExportResult::Exported {
                export_reference: event.event.forensic_event_id.to_string(),
            },
            Err(_) => ForensicExportResult::DependencyUnavailable {
                summary: "sqlite neuronforge forensic store write failed".to_owned(),
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
