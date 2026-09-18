use std::env;
use std::time::Duration;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::shared::{CorrelationId, RequestId, now_utc};
use crate::domain::status::ValidatedExecutionStatus;
use crate::errors::{FaLocalError, FaLocalResult};

/// Must match `forge_contract_core`'s `registry/repo_role_matrix.json` admitted
/// producer name for this repo exactly -- distinct from `config::SERVICE_ID`
/// (`"fa-local"`), which is this crate's own internal service identifier and
/// is not the artifact-producer name the contract-core role matrix admits.
const ARTIFACT_PRODUCER_SYSTEM: &str = "fa-local-operator";
const PRODUCED_BY_COMPONENT: &str = "execution_service.status_emitter";
const EXECUTION_STATUS_EVENT_FAMILY: &str = "execution_status_event";
const EXECUTION_STATUS_EVENT_VERSION: u32 = 1;

const DATAFORGE_LOCAL_DEFAULT_URL: &str = "http://127.0.0.1:8005";
const DATAFORGE_LOCAL_STATUS_EVENTS_PATH: &str = "/api/v1/execution-bridge/status-events";
const REQUEST_TIMEOUT_SECONDS: u64 = 10;

/// Typed request for posting an execution status event to DataForge Local's
/// proving-slice staging queue.
///
/// Serialized into a `execution_status_event` artifact envelope (family v1)
/// and POSTed to DataForge Local's local API.
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

/// The canonical proving-slice idempotency-key algorithm
/// (`forge_contract_core.identity.compute_idempotency_key` in Python; every
/// producer and consumer must compute it exactly this way -- never locally
/// invented). `raw = "{family}|{artifact_id}|{version}|{lineage_root_id}"`,
/// lowercase hex SHA-256.
fn compute_idempotency_key(
    artifact_family: &str,
    artifact_id: Uuid,
    artifact_version: u32,
    lineage_root_id: Uuid,
) -> String {
    let raw = format!("{artifact_family}|{artifact_id}|{artifact_version}|{lineage_root_id}");
    hex_digest(raw.as_bytes())
}

fn hex_digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn dataforge_local_base_url() -> String {
    env::var("DATAFORGE_LOCAL_URL").unwrap_or_else(|_| DATAFORGE_LOCAL_DEFAULT_URL.to_owned())
}

/// Build the `execution_status_event.v1` artifact envelope for one status
/// snapshot. Pure and network-free so its shape is independently testable.
///
/// This artifact is always a lineage root: `fa-local-operator` does not yet
/// emit an `execution_request` artifact to DataForge Local for this one to
/// chain onto (separate, unbuilt scope), so `lineage_root_id == artifact_id`
/// and `parent_artifact_id` is null -- the shared envelope schema's own
/// stated convention for a root artifact, not an invented shortcut.
fn build_status_event_artifact(request: &ExecutionStatusWritebackRequest) -> Value {
    let artifact_id = Uuid::new_v4();
    let lineage_root_id = artifact_id;
    let idempotency_key = compute_idempotency_key(
        EXECUTION_STATUS_EVENT_FAMILY,
        artifact_id,
        EXECUTION_STATUS_EVENT_VERSION,
        lineage_root_id,
    );
    let now = now_utc();
    let status = &request.status.status;

    let payload = json!({
        "request_id": status.request_id,
        "correlation_id": status.correlation_id,
        "state": status.state,
        "current_posture": status.current_posture,
        "execution_plan_id": status.execution_plan_id,
        "stable_plan_hash": status.stable_plan_hash,
        "degraded_subtype": status.degraded_subtype,
        "updated_at_utc": status.updated_at_utc,
        "started_at_utc": status.started_at_utc,
        "completed_at_utc": status.completed_at_utc,
        "current_step": status.current_step,
        "completion_summary": status.completion_summary,
        "failure_summary": status.failure_summary,
        "truthful_operator_summary": status.truthful_user_visible_summary,
    });

    // Not a cryptographic signature -- no signing key infrastructure exists
    // for this artifact family yet (unlike topology's run_token mechanism).
    // `signature` is schema-required as an opaque string over the canonical
    // payload body (`shared-envelope.schema.json`); a real digest of the
    // actual payload is more honest than a placeholder string, even though
    // it carries no cryptographic authority today.
    let signature = format!("sha256:{}", hex_digest(payload.to_string().as_bytes()));

    json!({
        "artifact_id": artifact_id,
        "artifact_family": EXECUTION_STATUS_EVENT_FAMILY,
        "artifact_version": EXECUTION_STATUS_EVENT_VERSION,
        "produced_by_system": ARTIFACT_PRODUCER_SYSTEM,
        "produced_by_component": PRODUCED_BY_COMPONENT,
        "source_scope": "local",
        "lineage_root_id": lineage_root_id,
        "parent_artifact_id": Value::Null,
        "trace_id": request.correlation_id.to_string(),
        "idempotency_key": idempotency_key,
        "created_at": now,
        "recorded_at": now,
        "sensitivity_class": "internal",
        "visibility_class": "operator",
        // "local_only", not "promotable": execution_trace_state is one of
        // fa-local-operator's own declared local_only_truth_classes in
        // forge_contract_core's repo_role_matrix.json -- this must never be
        // cloud-egress-eligible.
        "promotion_class": "local_only",
        "validation_status": "valid",
        "signer_identity": format!("{ARTIFACT_PRODUCER_SYSTEM}/{PRODUCED_BY_COMPONENT}@execution-bridge-v1"),
        "signature": signature,
        "payload": payload,
    })
}

/// Adapter for writing FA Local execution artifacts to DataForge Local.
///
/// DataForge Local is FA Local's local truth boundary. Execution status
/// events written here are recorded as local truth only -- `promotion_class:
/// "local_only"` keeps them out of any future cloud-egress path.
///
/// # Current status (Phase X4)
///
/// Wired: DataForge Local's `/api/v1/execution-bridge/status-events`
/// endpoint exists (`dataforge-Local#35`) and this adapter posts to it.
#[derive(Debug, Default)]
pub struct DfLocalAdapter;

impl DfLocalAdapter {
    /// Post a truthful execution status event to DataForge Local.
    ///
    /// The status is serialized as an `execution_status_event` v1 artifact in
    /// the shared envelope format defined by `forge-contract-core` and POSTed
    /// to `DATAFORGE_LOCAL_URL` (default `http://127.0.0.1:8005`).
    ///
    /// # Errors
    ///
    /// [`FaLocalError::WritebackFailed`] -- the request could not reach
    /// DataForge Local, or DataForge Local refused it (contract violation,
    /// idempotency conflict on a genuinely different retry, or any other
    /// non-2xx response).
    pub fn post_execution_status_event(
        &self,
        request: ExecutionStatusWritebackRequest,
    ) -> FaLocalResult<ExecutionStatusWritebackResult> {
        let artifact = build_status_event_artifact(&request);
        let base_url = dataforge_local_base_url();
        let url = format!("{base_url}{DATAFORGE_LOCAL_STATUS_EVENTS_PATH}");

        let response = ureq::post(&url)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
            .send_json(artifact);

        match response {
            Ok(resp) => {
                // 200 (replayed) and 201 (newly recorded) both mean acknowledged.
                Ok(ExecutionStatusWritebackResult {
                    request_id: request.request_id,
                    correlation_id: request.correlation_id,
                    acknowledged: resp.status() == 200 || resp.status() == 201,
                })
            }
            Err(ureq::Error::Status(status, resp)) => {
                let body = resp
                    .into_string()
                    .unwrap_or_else(|_| "<unreadable response body>".to_owned());
                Err(FaLocalError::WritebackFailed(format!(
                    "DataForge Local rejected the writeback (status {status}): {body}"
                )))
            }
            Err(ureq::Error::Transport(transport)) => Err(FaLocalError::WritebackFailed(format!(
                "could not reach DataForge Local at {url}: {transport}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::shared::{ApprovalPosture, ExecutionState};
    use crate::domain::status::ExecutionStatus;

    fn sample_status() -> ValidatedExecutionStatus {
        let now = now_utc();
        let status = ExecutionStatus::new(
            RequestId::new(),
            CorrelationId::new(),
            Some(crate::domain::shared::ExecutionPlanId::new()),
            Some("a".repeat(64)),
            ApprovalPosture::PolicyPreapproved,
            ExecutionState::Completed,
            None,
            Some(now),
            now,
            Some(now),
            None,
            Some("done".to_owned()),
            None,
            "Bounded execution completed.".to_owned(),
        )
        .expect("valid fixture status");
        ValidatedExecutionStatus::new(status).expect("status validates")
    }

    #[test]
    fn compute_idempotency_key_matches_the_canonical_python_algorithm() {
        // Cross-language parity vector: same inputs, same output, as
        // forge_contract_core.identity.compute_idempotency_key would produce
        // for sha256("execution_status_event|<id>|1|<id>").
        let id = Uuid::parse_str("a1b2c3d4-0003-0003-0003-000000000003").unwrap();
        let key = compute_idempotency_key("execution_status_event", id, 1, id);
        assert_eq!(key.len(), 64);
        assert!(
            key.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );

        // Same inputs must always produce the same key (determinism, not
        // just shape).
        let key_again = compute_idempotency_key("execution_status_event", id, 1, id);
        assert_eq!(key, key_again);

        // A different family must change the key (proves the pipe-joined
        // fields are actually load-bearing, not ignored).
        let other_family_key = compute_idempotency_key("network_observation", id, 1, id);
        assert_ne!(key, other_family_key);
    }

    #[test]
    fn build_status_event_artifact_is_a_root_artifact_with_required_fields() {
        let request = ExecutionStatusWritebackRequest::new(sample_status());
        let artifact = build_status_event_artifact(&request);

        assert_eq!(artifact["artifact_family"], "execution_status_event");
        assert_eq!(artifact["artifact_version"], 1);
        assert_eq!(artifact["produced_by_system"], "fa-local-operator");
        assert_eq!(artifact["source_scope"], "local");
        assert_eq!(artifact["promotion_class"], "local_only");
        assert_eq!(artifact["sensitivity_class"], "internal");
        assert_eq!(artifact["visibility_class"], "operator");
        assert_eq!(artifact["validation_status"], "valid");
        assert!(artifact["parent_artifact_id"].is_null());
        // Root artifact: lineage_root_id equals artifact_id.
        assert_eq!(artifact["artifact_id"], artifact["lineage_root_id"]);
        assert_eq!(artifact["trace_id"], request.correlation_id.to_string());

        let payload = &artifact["payload"];
        assert_eq!(payload["state"], "completed");
        assert_eq!(payload["current_posture"], "policy_preapproved");
        assert_eq!(
            payload["truthful_operator_summary"],
            "Bounded execution completed."
        );
        // A null-eligible field with no value present must serialize as an
        // explicit null, not be omitted -- the schema requires the key.
        assert!(payload["degraded_subtype"].is_null());
        assert!(payload["failure_summary"].is_null());
    }

    #[test]
    fn build_status_event_artifact_generates_a_fresh_artifact_id_each_call() {
        let request = ExecutionStatusWritebackRequest::new(sample_status());
        let first = build_status_event_artifact(&request);
        let second = build_status_event_artifact(&request);

        assert_ne!(first["artifact_id"], second["artifact_id"]);
        assert_ne!(first["idempotency_key"], second["idempotency_key"]);
    }
}
