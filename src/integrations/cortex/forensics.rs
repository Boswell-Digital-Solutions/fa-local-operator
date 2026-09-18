//! Truthful, bounded forensic events for Cortex Gnat dispatch runs.
//!
//! `forensic-event.schema.json` (`src/domain/forensics/mod.rs`) is built
//! entirely around FA Local's own execution-request domain --
//! `route_decision_id`, `execution_plan_id`, `ApprovalPosture`,
//! `ExecutionState` -- none of which a Cortex-initiated Gnat run has. This
//! is a separate, parallel contract for that domain instead of forcing
//! Gnat concepts through fields that don't fit them, matching how every
//! other Gnat contract (`gnat-dispatch-envelope`, `gnat-run-plan`,
//! `gnat-worker-receipt`, ...) already gets its own bounded schema rather
//! than reusing an execution-request one.
//!
//! [`GnatDispatchForensicEvent`] covers both events
//! [`GnatDispatchPipelineService`](crate::app::gnat_dispatch_pipeline_service::GnatDispatchPipelineService)
//! produces: the negotiation outcome (`GnatDispatchNegotiated`) and, for an
//! admitted run, each shard's own dispatch outcome
//! (`GnatShardDispatched`). Recording is currently in-memory only --
//! constructed, validated, and returned alongside the run outcome for the
//! caller to inspect or persist, the same way `execute`'s forensic records
//! existed before this repo's JSONL/SQLite export sinks did. Wiring a
//! matching export sink for this event family is a disclosed, separate
//! concern, not something this module does.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::shared::{
    CorrelationId, ForensicEventId, SchemaName, TimestampUtc, deserialize_contract_value,
};
use crate::errors::{FaLocalError, FaLocalResult};
use crate::integrations::cortex::GnatWorkerType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnatForensicEventType {
    GnatDispatchNegotiated,
    GnatShardDispatched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnatNegotiationOutcome {
    Denied,
    SerialFallbackPermitted,
    ReadyForFaLocalDispatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnatShardOutcome {
    Completed,
    NotCompleted,
    DispatchUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnatReceiptState {
    Complete,
    Denied,
    Stale,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GnatForensicRedactionLevel {
    None,
    SensitiveFieldsRedacted,
    LinkageOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GnatDispatchForensicEvent {
    pub forensic_event_id: ForensicEventId,
    pub correlation_id: CorrelationId,
    pub run_id: String,
    pub event_type: GnatForensicEventType,
    pub negotiation_outcome: GnatNegotiationOutcome,
    pub shard_id: Option<String>,
    pub worker_type: Option<GnatWorkerType>,
    pub shard_outcome: Option<GnatShardOutcome>,
    pub receipt_state: Option<GnatReceiptState>,
    pub timestamp_utc: TimestampUtc,
    pub summary: String,
    pub redaction_level: GnatForensicRedactionLevel,
    pub payload_minimized: bool,
}

impl GnatDispatchForensicEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        forensic_event_id: ForensicEventId,
        correlation_id: CorrelationId,
        run_id: String,
        event_type: GnatForensicEventType,
        negotiation_outcome: GnatNegotiationOutcome,
        shard_id: Option<String>,
        worker_type: Option<GnatWorkerType>,
        shard_outcome: Option<GnatShardOutcome>,
        receipt_state: Option<GnatReceiptState>,
        timestamp_utc: TimestampUtc,
        summary: String,
        redaction_level: GnatForensicRedactionLevel,
        payload_minimized: bool,
    ) -> FaLocalResult<Self> {
        let event = Self {
            forensic_event_id,
            correlation_id,
            run_id,
            event_type,
            negotiation_outcome,
            shard_id,
            worker_type,
            shard_outcome,
            receipt_state,
            timestamp_utc,
            summary,
            redaction_level,
            payload_minimized,
        };
        event.validate()?;
        Ok(event)
    }

    pub fn load_contract_value(value: &Value) -> FaLocalResult<Self> {
        deserialize_contract_value(SchemaName::GnatDispatchForensicEvent, value)
    }

    pub fn validate(&self) -> FaLocalResult<()> {
        if self.summary.is_empty() || self.summary.len() > 160 {
            return Err(contract_invalid(
                "gnat dispatch forensic event summary must be between 1 and 160 characters",
            ));
        }
        if self.run_id.is_empty() || self.run_id.len() > 120 {
            return Err(contract_invalid(
                "gnat dispatch forensic event run_id must be between 1 and 120 characters",
            ));
        }
        if !self.payload_minimized {
            return Err(contract_invalid(
                "gnat dispatch forensic event payload_minimized must remain true for bounded forensics",
            ));
        }

        match self.event_type {
            GnatForensicEventType::GnatDispatchNegotiated => {
                require_none_ref(self.shard_id.as_deref(), "shard_id")?;
                require_none(self.worker_type, "worker_type")?;
                require_none(self.shard_outcome, "shard_outcome")?;
                require_none(self.receipt_state, "receipt_state")?;
            }
            GnatForensicEventType::GnatShardDispatched => {
                if self.negotiation_outcome != GnatNegotiationOutcome::ReadyForFaLocalDispatch {
                    return Err(contract_invalid(
                        "gnat_shard_dispatched forensic event requires negotiation_outcome ready_for_fa_local_dispatch",
                    ));
                }
                require_some_ref(self.shard_id.as_deref(), "shard_id")?;
                require_some(self.worker_type, "worker_type")?;
                let shard_outcome = require_some(self.shard_outcome, "shard_outcome")?;

                match shard_outcome {
                    GnatShardOutcome::DispatchUnavailable => {
                        if self.receipt_state.is_some() {
                            return Err(contract_invalid(
                                "dispatch_unavailable gnat_shard_dispatched forensic event must not include receipt_state",
                            ));
                        }
                    }
                    GnatShardOutcome::Completed | GnatShardOutcome::NotCompleted => {
                        require_some(self.receipt_state, "receipt_state")?;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn validated(self) -> FaLocalResult<ValidatedGnatDispatchForensicEvent> {
        self.validate()?;
        Ok(ValidatedGnatDispatchForensicEvent { event: self })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedGnatDispatchForensicEvent {
    pub event: GnatDispatchForensicEvent,
}

impl ValidatedGnatDispatchForensicEvent {
    pub fn new(event: GnatDispatchForensicEvent) -> FaLocalResult<Self> {
        event.validated()
    }
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}

fn require_none<T>(value: Option<T>, field: &'static str) -> FaLocalResult<()> {
    if value.is_none() {
        Ok(())
    } else {
        Err(contract_invalid(format!(
            "gnat_dispatch_negotiated forensic event must not include {field}"
        )))
    }
}

fn require_none_ref<T: ?Sized>(value: Option<&T>, field: &'static str) -> FaLocalResult<()> {
    if value.is_none() {
        Ok(())
    } else {
        Err(contract_invalid(format!(
            "gnat_dispatch_negotiated forensic event must not include {field}"
        )))
    }
}

fn require_some<T>(value: Option<T>, field: &'static str) -> FaLocalResult<T> {
    value.ok_or_else(|| {
        contract_invalid(format!(
            "gnat_shard_dispatched forensic event must include {field}"
        ))
    })
}

fn require_some_ref<'a, T: ?Sized>(value: Option<&'a T>, field: &'static str) -> FaLocalResult<()> {
    if value.is_some() {
        Ok(())
    } else {
        Err(contract_invalid(format!(
            "gnat_shard_dispatched forensic event must include {field}"
        )))
    }
}
