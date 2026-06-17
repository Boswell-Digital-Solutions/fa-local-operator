use fa_local::errors::FaLocalError;
use fa_local::integrations::neuronforge_local::{
    InferenceMessage, LocalInferenceRequest, LocalInferenceResult, LocalInferenceTransport,
    NeuronForgeLocalAdapter, NeuronForgeLocalConfig, SamplingParameters, StopReason, TokenUsage,
};
use fa_local::{CapabilityId, CorrelationId, EnvironmentMode, RequestId};

const MODEL: &str = "neuronforge-local-7b";

fn config() -> NeuronForgeLocalConfig {
    NeuronForgeLocalConfig::new(
        "http://127.0.0.1:8757/infer",
        vec![MODEL.to_owned()],
        2048,
        EnvironmentMode::Dev,
    )
}

fn request() -> LocalInferenceRequest {
    LocalInferenceRequest {
        request_id: RequestId::new(),
        correlation_id: CorrelationId::new(),
        capability_id: CapabilityId::new(),
        model_id: MODEL.to_owned(),
        messages: vec![
            InferenceMessage::system("You are a bounded local operator."),
            InferenceMessage::user("Summarize the local execution doctrine."),
        ],
        sampling: SamplingParameters::bounded_default(256),
    }
}

#[derive(Debug)]
struct EchoTransport;

impl LocalInferenceTransport for EchoTransport {
    fn transport_id(&self) -> &'static str {
        "echo-transport"
    }

    fn submit(&self, request: &LocalInferenceRequest) -> Result<LocalInferenceResult, FaLocalError> {
        Ok(LocalInferenceResult {
            request_id: request.request_id,
            correlation_id: request.correlation_id,
            model_id: request.model_id.clone(),
            output_text: "bounded local completion".to_owned(),
            stop_reason: StopReason::EndOfTurn,
            usage: TokenUsage {
                prompt_tokens: 12,
                completion_tokens: 4,
            },
        })
    }
}

/// A transport that ignores the request lineage and answers for a different run.
#[derive(Debug)]
struct LineageBreakingTransport;

impl LocalInferenceTransport for LineageBreakingTransport {
    fn transport_id(&self) -> &'static str {
        "lineage-breaking-transport"
    }

    fn submit(&self, request: &LocalInferenceRequest) -> Result<LocalInferenceResult, FaLocalError> {
        Ok(LocalInferenceResult {
            request_id: RequestId::new(),
            correlation_id: request.correlation_id,
            model_id: request.model_id.clone(),
            output_text: "wrong run".to_owned(),
            stop_reason: StopReason::EndOfTurn,
            usage: TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
            },
        })
    }
}

#[test]
fn unwired_adapter_fails_closed_with_inference_not_wired() {
    let adapter = NeuronForgeLocalAdapter::new(config());
    assert!(!adapter.is_wired());
    assert_eq!(adapter.adapter_id(), "neuronforge-local-operator");

    let error = adapter.submit_inference(&request()).unwrap_err();
    assert!(matches!(error, FaLocalError::InferenceNotWired(_)));
}

#[test]
fn wired_adapter_delivers_admitted_request_to_transport() {
    let adapter = NeuronForgeLocalAdapter::with_transport(config(), EchoTransport);
    assert!(adapter.is_wired());

    let req = request();
    let result = adapter.submit_inference(&req).unwrap();

    assert_eq!(result.request_id, req.request_id);
    assert_eq!(result.correlation_id, req.correlation_id);
    assert_eq!(result.model_id, MODEL);
    assert_eq!(result.output_text, "bounded local completion");
    assert_eq!(result.stop_reason, StopReason::EndOfTurn);
    assert_eq!(result.usage.total(), 16);
}

#[test]
fn admission_runs_before_transport_so_disallowed_model_is_denied() {
    let adapter = NeuronForgeLocalAdapter::with_transport(config(), EchoTransport);
    let mut req = request();
    req.model_id = "some-cloud-model".to_owned();

    let error = adapter.submit_inference(&req).unwrap_err();
    assert!(matches!(error, FaLocalError::Denied(_)));
    assert_eq!(
        error.to_string(),
        "local inference request names a model not permitted by adapter config"
    );
}

#[test]
fn empty_context_is_denied_fail_closed() {
    let adapter = NeuronForgeLocalAdapter::with_transport(config(), EchoTransport);
    let mut req = request();
    req.messages.clear();

    let error = adapter.submit_inference(&req).unwrap_err();
    assert!(matches!(error, FaLocalError::Denied(_)));
    assert_eq!(
        error.to_string(),
        "local inference request must carry at least one message"
    );
}

#[test]
fn context_without_user_message_is_denied() {
    let adapter = NeuronForgeLocalAdapter::with_transport(config(), EchoTransport);
    let mut req = request();
    req.messages = vec![InferenceMessage::system("only framing, no user turn")];

    let error = adapter.submit_inference(&req).unwrap_err();
    assert_eq!(
        error.to_string(),
        "local inference request must carry at least one user message"
    );
}

#[test]
fn out_of_range_sampling_is_denied() {
    let adapter = NeuronForgeLocalAdapter::with_transport(config(), EchoTransport);

    let mut hot = request();
    hot.sampling.temperature = 3.5;
    assert_eq!(
        adapter.submit_inference(&hot).unwrap_err().to_string(),
        "local inference temperature is out of the permitted range 0.0..=2.0"
    );

    let mut zero_tokens = request();
    zero_tokens.sampling.max_output_tokens = 0;
    assert_eq!(
        adapter.submit_inference(&zero_tokens).unwrap_err().to_string(),
        "local inference request must permit at least one output token"
    );
}

#[test]
fn max_output_tokens_above_ceiling_is_denied() {
    let adapter = NeuronForgeLocalAdapter::with_transport(config(), EchoTransport);
    let mut req = request();
    req.sampling.max_output_tokens = 9999;

    assert_eq!(
        adapter.submit_inference(&req).unwrap_err().to_string(),
        "local inference max_output_tokens exceeds adapter ceiling"
    );
}

#[test]
fn transport_breaking_lineage_is_rejected_as_internal_invariant() {
    let adapter = NeuronForgeLocalAdapter::with_transport(config(), LineageBreakingTransport);
    let error = adapter.submit_inference(&request()).unwrap_err();

    assert!(matches!(error, FaLocalError::InternalInvariant(_)));
    assert_eq!(
        error.to_string(),
        "internal invariant violated: NeuronForge Local transport returned result with mismatched lineage"
    );
}
