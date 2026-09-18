pub mod jsonl_forensic_export;
pub mod sqlite_forensic_store;

use crate::domain::forensics::ValidatedForensicEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForensicExportReceipt {
    pub adapter_id: String,
    pub export_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForensicExportResult {
    Exported { export_reference: String },
    DependencyUnavailable { summary: String },
    Unsupported { summary: String },
}

pub trait ForensicEventExportAdapter {
    fn adapter_id(&self) -> &'static str;

    fn export_event(&self, event: &ValidatedForensicEvent) -> ForensicExportResult;
}
