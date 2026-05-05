use serde_json::Value;

use crate::domain::execution::ExecutionRequest;
use crate::errors::FaLocalResult;

/// Result of a successful intake validation.
#[derive(Debug, Clone)]
pub struct IntakeValidationResult {
    /// The typed, schema-validated execution request.
    pub request: ExecutionRequest,
}

/// Entry point for all execution requests into FA Local.
///
/// The intake service validates incoming requests against FA Local's execution
/// request contract schema before they may enter the policy/capability admission
/// pipeline. Only requests that pass intake validation can proceed.
///
/// # Contract boundary
///
/// The intake service enforces:
/// - Schema validity (via FA Local's execution request contract schema)
/// - Typed field extraction (request_id, correlation_id, requester_id, etc.)
///
/// What it does NOT enforce:
/// - Requester trust (handled by requester trust domain)
/// - Policy admission (handled by policy domain)
/// - Capability availability (handled by capability registry)
///
/// Those checks happen downstream after intake validation passes.
#[derive(Debug, Default)]
pub struct IntakeService;

impl IntakeService {
    /// Validate raw JSON as a bounded execution request.
    ///
    /// Validates the input against FA Local's execution request contract schema
    /// and returns the typed [`ExecutionRequest`] on success.
    ///
    /// # Errors
    ///
    /// Returns a contract error if the input fails schema validation or cannot
    /// be deserialized into the expected execution request structure.
    pub fn validate_request(&self, value: &Value) -> FaLocalResult<IntakeValidationResult> {
        let request = ExecutionRequest::load_contract_value(value)?;
        Ok(IntakeValidationResult { request })
    }

    /// Validate raw JSON bytes as a bounded execution request.
    ///
    /// Parses the JSON bytes and then validates as an execution request.
    /// Convenience method for callers that have raw byte slices (e.g., from file or stdin).
    pub fn validate_request_bytes(&self, bytes: &[u8]) -> FaLocalResult<IntakeValidationResult> {
        let value: Value = serde_json::from_slice(bytes)?;
        self.validate_request(&value)
    }
}
