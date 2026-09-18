use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::adapters::exports::{ForensicEventExportAdapter, ForensicExportResult};
use crate::domain::forensics::{ForensicEvent, ForensicEventType, ValidatedForensicEvent};
use crate::domain::shared::CorrelationId;
use crate::errors::FaLocalResult;

const CREATE_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS forensic_events (
    forensic_event_id TEXT PRIMARY KEY,
    correlation_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    execution_state TEXT NOT NULL,
    timestamp_utc TEXT NOT NULL,
    event_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_forensic_events_correlation_id ON forensic_events(correlation_id);
CREATE INDEX IF NOT EXISTS idx_forensic_events_event_type ON forensic_events(event_type);
";

/// Queryable local forensic store, backed by a bundled SQLite database.
///
/// Indexes `correlation_id` and `event_type` for lookup; the full event is
/// stored as its own already-minimal JSON encoding (`ForensicEvent` is
/// content-sparing by construction: bounded summaries, `payload_minimized`
/// always true) and round-tripped through the same `Serialize`/`Deserialize`
/// derives used everywhere else, rather than hand-mapped into a wide
/// relational schema.
#[derive(Debug)]
pub struct SqliteForensicStore {
    connection: Mutex<Connection>,
}

impl SqliteForensicStore {
    pub fn open(path: &Path) -> FaLocalResult<Self> {
        let connection = Connection::open(path)?;
        connection.execute_batch(CREATE_SCHEMA)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn query_by_correlation_id(
        &self,
        correlation_id: CorrelationId,
    ) -> FaLocalResult<Vec<ForensicEvent>> {
        self.query_where("correlation_id = ?1", params![correlation_id.to_string()])
    }

    pub fn query_by_event_type(
        &self,
        event_type: ForensicEventType,
    ) -> FaLocalResult<Vec<ForensicEvent>> {
        self.query_where("event_type = ?1", params![enum_text(&event_type)])
    }

    fn query_where(
        &self,
        predicate: &str,
        query_params: impl rusqlite::Params,
    ) -> FaLocalResult<Vec<ForensicEvent>> {
        let connection = self
            .connection
            .lock()
            .expect("sqlite connection mutex poisoned");
        let mut statement = connection.prepare(&format!(
            "SELECT event_json FROM forensic_events WHERE {predicate} ORDER BY timestamp_utc ASC"
        ))?;
        let rows = statement.query_map(query_params, |row| row.get::<_, String>(0))?;

        let mut events = Vec::new();
        for row in rows {
            events.push(serde_json::from_str(&row?)?);
        }
        Ok(events)
    }

    fn insert(&self, event: &ForensicEvent) -> FaLocalResult<()> {
        let connection = self
            .connection
            .lock()
            .expect("sqlite connection mutex poisoned");
        let event_json = serde_json::to_string(event)?;

        connection.execute(
            "INSERT INTO forensic_events
                (forensic_event_id, correlation_id, request_id, event_type, execution_state, timestamp_utc, event_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                event.forensic_event_id.to_string(),
                event.correlation_id.to_string(),
                event.request_id.to_string(),
                enum_text(&event.event_type),
                enum_text(&event.execution_state),
                event.timestamp_utc.to_rfc3339(),
                event_json,
            ],
        )?;

        Ok(())
    }
}

impl ForensicEventExportAdapter for SqliteForensicStore {
    fn adapter_id(&self) -> &'static str {
        "sqlite-forensic-store"
    }

    fn export_event(&self, event: &ValidatedForensicEvent) -> ForensicExportResult {
        match self.insert(&event.event) {
            Ok(()) => ForensicExportResult::Exported {
                export_reference: event.event.forensic_event_id.to_string(),
            },
            Err(_) => ForensicExportResult::DependencyUnavailable {
                summary: "sqlite forensic store write failed".to_owned(),
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
