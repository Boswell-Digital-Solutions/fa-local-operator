//! NeuronForge Local integration — FA Local's bounded local inference boundary.
//!
//! NeuronForge Local is the local large-language-model operator that backs
//! FA Local's inference-class capabilities. FA Local does not host model
//! weights or run inference itself; it governs *whether* a bounded inference
//! request is admitted, then hands the admitted request to NeuronForge Local
//! across the contract defined here.
//!
//! ```text
//! requester → FA Local intake / admission / routing
//!           → NeuronForge Local adapter (this module)   ← governed seam
//!           → NeuronForge Local inference runtime (local model)
//! ```
//!
//! # Design
//!
//! The adapter keeps three concerns explicit and separate:
//!
//! - **Contract** — the typed [`LocalInferenceRequest`] / [`LocalInferenceResult`]
//!   vocabulary that crosses the boundary. This is stable and does not depend on
//!   any particular transport.
//! - **Transport** — the [`LocalInferenceTransport`] trait. A real client (HTTP
//!   to a local NeuronForge Local endpoint, an in-process runtime, or a test
//!   double) implements this trait and is injected via
//!   [`NeuronForgeLocalAdapter::with_transport`].
//! - **Admission** — fail-closed validation performed by the adapter before any
//!   transport is touched. An adapter constructed without a transport
//!   ([`NeuronForgeLocalAdapter::new`]) is *unwired* and refuses every request
//!   with [`FaLocalError::InferenceNotWired`].
//!
//! This mirrors the trait-based boundary used by the execution-delivery
//! adapters and the not-yet-wired posture of the DataForge Local adapter: the
//! contract is real today, the default transport is fail-closed, and a live
//! client can be plugged in without reshaping callers.

use crate::domain::guards::deny;
use crate::domain::shared::{
    CapabilityId, CorrelationId, DenialBasis, DenialReasonClass, DenialScope, EnvironmentMode,
    RequestId,
};
use crate::errors::{FaLocalError, FaLocalResult};

/// The conversational role of a single inference message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InferenceRole {
    /// System / instruction framing established by FA Local, not the requester.
    System,
    /// Content originating from the admitted requester.
    User,
    /// Prior model output replayed back into the context window.
    Assistant,
}

/// A single message in the bounded inference context window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceMessage {
    pub role: InferenceRole,
    pub content: String,
}

impl InferenceMessage {
    pub fn new(role: InferenceRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(InferenceRole::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(InferenceRole::User, content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(InferenceRole::Assistant, content)
    }
}

/// Bounded sampling parameters for a local inference request.
///
/// These are intentionally narrow and explicit. The adapter rejects out-of-range
/// values fail-closed rather than silently clamping them, so an over-broad
/// request never reaches the model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplingParameters {
    /// Sampling temperature, constrained to `0.0..=2.0`.
    pub temperature: f32,
    /// Nucleus sampling cutoff, constrained to `0.0..=1.0`.
    pub top_p: f32,
    /// Hard ceiling on generated tokens. Must be non-zero.
    pub max_output_tokens: u32,
    /// Optional deterministic seed for reproducible local runs.
    pub seed: Option<u64>,
}

impl SamplingParameters {
    /// Conservative, deterministic defaults suitable for governed local runs.
    pub fn bounded_default(max_output_tokens: u32) -> Self {
        Self {
            temperature: 0.2,
            top_p: 0.9,
            max_output_tokens,
            seed: None,
        }
    }
}

/// A bounded request for local inference, carrying full lineage.
///
/// The `request_id` and `correlation_id` are the same identifiers used across
/// FA Local intake, routing, and forensics, so an inference call is traceable
/// end-to-end. The `capability_id` records which admitted capability authorized
/// this inference.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalInferenceRequest {
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub capability_id: CapabilityId,
    /// Requested local model identifier (must be permitted by adapter config).
    pub model_id: String,
    /// Ordered context window for the request.
    pub messages: Vec<InferenceMessage>,
    pub sampling: SamplingParameters,
}

/// Why local generation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StopReason {
    /// The model produced a natural end-of-turn.
    EndOfTurn,
    /// Generation hit the `max_output_tokens` ceiling.
    MaxOutputTokens,
    /// A configured stop sequence was emitted.
    StopSequence,
    /// The request was canceled before completion.
    Canceled,
}

/// Token accounting reported by the local runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
}

impl TokenUsage {
    pub fn total(&self) -> u32 {
        self.prompt_tokens.saturating_add(self.completion_tokens)
    }
}

/// The validated result of a local inference request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalInferenceResult {
    pub request_id: RequestId,
    pub correlation_id: CorrelationId,
    pub model_id: String,
    pub output_text: String,
    pub stop_reason: StopReason,
    pub usage: TokenUsage,
}

/// Transport seam for delivering an admitted inference request to a local model.
///
/// Implementors carry whatever is needed to reach the NeuronForge Local runtime
/// (an HTTP client to a local endpoint, an in-process handle, or a test double).
/// The adapter performs fail-closed admission *before* calling [`submit`], so a
/// transport may assume the request has already passed contract validation —
/// but it remains responsible for honoring the request's bounds at runtime.
///
/// [`submit`]: LocalInferenceTransport::submit
pub trait LocalInferenceTransport: std::fmt::Debug {
    /// Stable identifier for this transport, used in diagnostics and lineage.
    fn transport_id(&self) -> &'static str;

    /// Deliver an admitted request to the local model and return its result.
    fn submit(&self, request: &LocalInferenceRequest) -> FaLocalResult<LocalInferenceResult>;
}

/// Configuration for the NeuronForge Local boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeuronForgeLocalConfig {
    /// Local endpoint the transport will reach (e.g. a loopback URL). Recorded
    /// for diagnostics; the transport owns how it is used.
    pub endpoint: String,
    /// Model identifiers this adapter is permitted to route to. Empty means no
    /// model is admitted, and every request is denied fail-closed.
    pub allowed_model_ids: Vec<String>,
    /// Hard ceiling on `max_output_tokens` across all requests.
    pub max_output_tokens_ceiling: u32,
    /// Environment the adapter is operating in.
    pub environment_mode: EnvironmentMode,
}

impl NeuronForgeLocalConfig {
    pub fn new(
        endpoint: impl Into<String>,
        allowed_model_ids: Vec<String>,
        max_output_tokens_ceiling: u32,
        environment_mode: EnvironmentMode,
    ) -> Self {
        Self {
            endpoint: endpoint.into(),
            allowed_model_ids,
            max_output_tokens_ceiling,
            environment_mode,
        }
    }

    fn permits_model(&self, model_id: &str) -> bool {
        self.allowed_model_ids
            .iter()
            .any(|allowed| allowed == model_id)
    }
}

/// Adapter governing FA Local's access to the NeuronForge Local model runtime.
///
/// Construct with [`NeuronForgeLocalAdapter::new`] for the fail-closed, unwired
/// posture (every request returns [`FaLocalError::InferenceNotWired`]), or with
/// [`NeuronForgeLocalAdapter::with_transport`] to attach a live transport.
#[derive(Debug)]
pub struct NeuronForgeLocalAdapter {
    config: NeuronForgeLocalConfig,
    transport: Option<Box<dyn LocalInferenceTransport>>,
}

impl NeuronForgeLocalAdapter {
    /// Stable identifier for this integration boundary.
    pub const ADAPTER_ID: &'static str = "neuronforge-local-operator";

    /// Build an unwired adapter. Admission still runs, but every otherwise-valid
    /// request returns [`FaLocalError::InferenceNotWired`] because no transport
    /// is attached.
    pub fn new(config: NeuronForgeLocalConfig) -> Self {
        Self {
            config,
            transport: None,
        }
    }

    /// Build a wired adapter backed by a live inference transport.
    pub fn with_transport(
        config: NeuronForgeLocalConfig,
        transport: impl LocalInferenceTransport + 'static,
    ) -> Self {
        Self {
            config,
            transport: Some(Box::new(transport)),
        }
    }

    pub fn adapter_id(&self) -> &'static str {
        Self::ADAPTER_ID
    }

    pub fn config(&self) -> &NeuronForgeLocalConfig {
        &self.config
    }

    /// Whether a transport is attached. An unwired adapter cannot deliver work.
    pub fn is_wired(&self) -> bool {
        self.transport.is_some()
    }

    /// Admit and deliver a bounded local inference request.
    ///
    /// The request is validated fail-closed first. Only if it passes is it handed
    /// to the attached transport. The returned [`LocalInferenceResult`] is checked
    /// to ensure the transport preserved request lineage, so a transport cannot
    /// silently answer for a different request.
    ///
    /// # Errors
    ///
    /// - [`FaLocalError::Denied`] — the request failed fail-closed admission
    ///   (empty context, disallowed model, out-of-range sampling, etc.).
    /// - [`FaLocalError::InferenceNotWired`] — the adapter has no transport.
    /// - Any error surfaced by the transport.
    /// - [`FaLocalError::InternalInvariant`] — the transport returned a result
    ///   whose lineage does not match the request.
    pub fn submit_inference(
        &self,
        request: &LocalInferenceRequest,
    ) -> FaLocalResult<LocalInferenceResult> {
        self.validate_request(request)?;

        let Some(transport) = self.transport.as_ref() else {
            return Err(FaLocalError::InferenceNotWired(format!(
                "NeuronForge Local transport not attached for endpoint {:?}",
                self.config.endpoint
            )));
        };

        let result = transport.submit(request)?;

        if result.request_id != request.request_id
            || result.correlation_id != request.correlation_id
        {
            return Err(FaLocalError::InternalInvariant(
                "NeuronForge Local transport returned result with mismatched lineage".to_owned(),
            ));
        }

        if result.model_id != request.model_id {
            return Err(FaLocalError::InternalInvariant(
                "NeuronForge Local transport returned result for a different model".to_owned(),
            ));
        }

        Ok(result)
    }

    /// Fail-closed contract validation applied before any transport call.
    fn validate_request(&self, request: &LocalInferenceRequest) -> FaLocalResult<()> {
        if request.messages.is_empty() {
            return Err(deny(
                DenialReasonClass::ContractInvalid,
                DenialScope::Request,
                DenialBasis::Contract,
                "local inference request must carry at least one message",
            )
            .into());
        }

        if !request
            .messages
            .iter()
            .any(|message| message.role == InferenceRole::User)
        {
            return Err(deny(
                DenialReasonClass::ContractInvalid,
                DenialScope::Request,
                DenialBasis::Contract,
                "local inference request must carry at least one user message",
            )
            .into());
        }

        if request
            .messages
            .iter()
            .any(|message| message.content.trim().is_empty())
        {
            return Err(deny(
                DenialReasonClass::ContractInvalid,
                DenialScope::Request,
                DenialBasis::Contract,
                "local inference request must not carry empty message content",
            )
            .into());
        }

        if request.model_id.trim().is_empty() {
            return Err(deny(
                DenialReasonClass::ContractInvalid,
                DenialScope::Request,
                DenialBasis::Contract,
                "local inference request must name a model",
            )
            .into());
        }

        if !self.config.permits_model(&request.model_id) {
            return Err(deny(
                DenialReasonClass::CapabilityNotAdmitted,
                DenialScope::Capability,
                DenialBasis::Policy,
                "local inference request names a model not permitted by adapter config",
            )
            .into());
        }

        let sampling = &request.sampling;
        if !(0.0..=2.0).contains(&sampling.temperature) {
            return Err(deny(
                DenialReasonClass::ContractInvalid,
                DenialScope::Request,
                DenialBasis::Contract,
                "local inference temperature is out of the permitted range 0.0..=2.0",
            )
            .into());
        }

        if !(0.0..=1.0).contains(&sampling.top_p) {
            return Err(deny(
                DenialReasonClass::ContractInvalid,
                DenialScope::Request,
                DenialBasis::Contract,
                "local inference top_p is out of the permitted range 0.0..=1.0",
            )
            .into());
        }

        if sampling.max_output_tokens == 0 {
            return Err(deny(
                DenialReasonClass::ContractInvalid,
                DenialScope::Request,
                DenialBasis::Contract,
                "local inference request must permit at least one output token",
            )
            .into());
        }

        if sampling.max_output_tokens > self.config.max_output_tokens_ceiling {
            return Err(deny(
                DenialReasonClass::ContractInvalid,
                DenialScope::Request,
                DenialBasis::Contract,
                "local inference max_output_tokens exceeds adapter ceiling",
            )
            .into());
        }

        Ok(())
    }
}
