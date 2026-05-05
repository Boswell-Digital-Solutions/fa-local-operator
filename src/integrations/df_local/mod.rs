use crate::domain::shared::{CorrelationId, RequestId};
use crate::domain::status::ValidatedExecutionStatus;
use crate::errors::{FaLocalError, FaLocalResult};

/// Typed request for posting an execution status event to DataForge Local's
/// proving-slice staging queue.
///
/// When wired, the adapter will serialize this into a `execution_status_event`
/// artifact envelope (family v1) and POST it to the DataForge Local local API.
#[derive(Debug, Clone)]
pub struct ExecutionStatusWritebackRequest {
    /// Stable request ID from the original execution request.
    pub request_id: RequestId,
    /// Lineage correlation ID spanning the full execution chain.
    pub correlation_id: CorrelationId,
    /// The validated execution status snapshot to write back.
    pub status: ValidatedExecutionStatus,
}

impl ExecutionStatusWritebackRequest {
    pub fn new(status: ValidatedExecutionStatus) -> Self {
        Self {
            request_id: status.status.request_id,
            correlation_id: status.status.correlation_id,
            status,
        }
    }
}

/// Result of a successful execution status writeback to DataForge Local.
#[derive(Debug, Clone)]
pub struct ExecutionStatusWritebackResult {
    /// The request ID that was acknowledged.
    pub request_id: RequestId,
    /// The correlation ID that was acknowledged.
    pub correlation_id: CorrelationId,
    /// Whether the writeback was durably acknowledged by DataForge Local.
    pub acknowledged: bool,
}

/// Adapter for writing FA Local execution artifacts to DataForge Local.
///
/// DataForge Local is FA Local's local truth boundary. Execution status events
/// written here travel through the proving-slice pipeline:
///
/// ```text
/// FA Local → DataForge Local staging queue
///          → DataForge Cloud intake
///          → Forge_Command execution review surface
/// ```
///
/// This follows the same path as `source_drift_finding` artifacts in proving
/// slice 01 — FA Local is the execution producer; DataForge Local is the
/// local promoter; DataForge Cloud is the shared truth owner; Forge_Command
/// is the review surface.
///
/// # Current status
///
/// Not yet wired. DataForge Local does not yet have an `execution_status_event`
/// staging endpoint. Methods return `FaLocalError::WritebackNotWired` until
/// the DataForge Local endpoint is implemented and HTTP transport is added.
#[derive(Debug, Default)]
pub struct DfLocalAdapter;

impl DfLocalAdapter {
    /// Post a truthful execution status event to the DataForge Local staging queue.
    ///
    /// The status is serialized as an `execution_status_event` v1 artifact in the
    /// shared envelope format defined by `forge-contract-core`. It is posted to
    /// DataForge Local's local staging endpoint, from which it is promoted to
    /// DataForge Cloud and becomes visible in Forge_Command's execution review surface.
    ///
    /// # Serialization contract
    ///
    /// The artifact envelope fields are:
    /// - `artifact_family`: `"execution_status_event"`
    /// - `artifact_version`: `1`
    /// - `produced_by_system`: `"fa-local-operator"`
    /// - `produced_by_component`: `"execution_service.status_emitter"`
    /// - `source_scope`: `"local"`
    /// - `promotion_class`: `"promotable"`
    /// - `payload`: fields mapped from [`ValidatedExecutionStatus`]
    ///   (see `contracts/families/execution_status_event/execution_status_event.v1.schema.json`
    ///   in `forge-contract-core`)
    ///
    /// # Errors
    ///
    /// - [`FaLocalError::WritebackNotWired`] — DataForge Local endpoint not yet implemented.
    ///   This is the expected return until Phase X4 completes the real transport.
    pub fn post_execution_status_event(
        &self,
        request: ExecutionStatusWritebackRequest,
    ) -> FaLocalResult<ExecutionStatusWritebackResult> {
        // WRITEBACK NOT YET WIRED — Phase X3 establishes the contract boundary.
        //
        // Phase X4 will complete this by:
        // 1. Adding the execution_status_event staging endpoint to DataForge Local
        // 2. Adding an HTTP client dependency (e.g., reqwest or ureq)
        // 3. Replacing this error with the real serialization + POST logic
        //
        // The serialization target is the execution_status_event v1 schema
        // in forge-contract-core/contracts/families/execution_status_event/.
        let _ = request;
        Err(FaLocalError::WritebackNotWired(
            "DataForge Local execution_status_event staging endpoint not yet implemented".to_owned(),
        ))
    }
}
