//! Dispatches the one bounded task NeuronForge Local's `ADR-002`
//! (`neuronforge-local-operator` repo, `docs/adr/ADR-002-fa-local-task-routing.md`)
//! authorizes -- `analyze.style.scene.v1` -- to its
//! `POST /api/v1/fa-local/task-dispatch` route and reports back the
//! truthful outcome.
//!
//! ADR-002 assigns FA Local routing ownership for this task: "FA-Local
//! decides when, whether, and with what bounded parameters to invoke the
//! admitted task." No other task is admitted; [`HttpNeuronForgeLocalAdapter::dispatch_task`]
//! refuses anything else before ever making a network call, the same way
//! Cortex's `AUTHORIZED_WORKER_TYPES` check happens before spawning a
//! process (`integrations::cortex::shard_dispatch`).
//!
//! Every response NeuronForge Local returns already carries
//! `semantic_result_posture: "non_canonical_candidate"` and guardrails
//! proving it cannot mutate registry state or promote a baseline (ADR-002's
//! output-posture section) -- FA Local does not re-derive or enforce that
//! posture itself, it is NeuronForge Local's own structural guarantee, and
//! this adapter passes the receipt through unmodified.
//!
//! Transport is HTTP, not a spawned subprocess: ADR-002's own transport
//! section explains why (FA Local dispatching as an HTTP *client* does not
//! conflict with its own "no HTTP surface" doctrine, which constrains what
//! it *serves*) -- the same `ureq`-based pattern `integrations::df_local`
//! already uses against DataForge Local.

use std::env;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The one task [`ADR-002`](self) admits for FA-Local-initiated dispatch.
/// Checked before ever making a network call, not left for NeuronForge
/// Local's own route to reject.
pub const ADMITTED_TASK_ID: &str = "analyze.style.scene.v1";

const NEURONFORGE_LOCAL_DEFAULT_URL: &str = "http://127.0.0.1:8000";
const NEURONFORGE_LOCAL_TASK_DISPATCH_PATH: &str = "/api/v1/fa-local/task-dispatch";
const REQUEST_TIMEOUT_SECONDS: u64 = 180;

/// Model-resource disclosure fields, matching NeuronForge Local's own
/// `ModelResourceDisclosure` (`service/cor_gnat_semantic_handoff.py`) --
/// the same shape its `FaLocalTaskDispatchRequest` requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelResourceDisclosure {
    pub route_class: String,
    pub model_id: String,
    pub resource_budget_class: String,
    pub execution_mode: String,
}

/// Everything needed to dispatch one bounded NeuronForge Local task.
#[derive(Debug, Clone)]
pub struct NeuronForgeTaskDispatchRequest {
    pub dispatch_id: String,
    pub request_id: String,
    pub task_id: String,
    pub scene_text: String,
    pub model_resource_disclosure: ModelResourceDisclosure,
    pub operator_visible_message: String,
}

/// The truthful outcome of one dispatch attempt.
#[derive(Debug, Clone)]
pub enum NeuronForgeTaskDispatchResult {
    /// NeuronForge Local produced a real receipt and
    /// `schema_validation_status: "valid"`.
    Completed { receipt: Value },
    /// NeuronForge Local produced a real receipt, but the result did not
    /// validate cleanly (`"degraded"` or `"failed"`) -- a truthful outcome,
    /// not a dispatch failure.
    NotCompleted { receipt: Value },
    /// No receipt could be obtained at all: the task is not admitted, the
    /// request could not reach NeuronForge Local, or it returned something
    /// that could not be parsed as a receipt.
    DispatchUnavailable { summary: String },
}

pub trait NeuronForgeTaskDeliveryAdapter {
    fn adapter_id(&self) -> &'static str;

    fn dispatch_task(
        &self,
        request: &NeuronForgeTaskDispatchRequest,
    ) -> NeuronForgeTaskDispatchResult;
}

fn neuronforge_local_base_url() -> String {
    env::var("NEURONFORGE_LOCAL_URL").unwrap_or_else(|_| NEURONFORGE_LOCAL_DEFAULT_URL.to_owned())
}

/// Config for [`HttpNeuronForgeLocalAdapter`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HttpNeuronForgeLocalAdapterConfig {
    /// Overrides `NEURONFORGE_LOCAL_URL`/the default base URL when set --
    /// mainly for tests pointed at a local mock server.
    pub base_url_override: Option<String>,
}

impl HttpNeuronForgeLocalAdapterConfig {
    pub fn new(base_url_override: Option<String>) -> Self {
        Self { base_url_override }
    }
}

/// Dispatches the one admitted task to a real NeuronForge Local process over
/// HTTP.
#[derive(Debug, Clone, Default)]
pub struct HttpNeuronForgeLocalAdapter {
    config: HttpNeuronForgeLocalAdapterConfig,
}

impl HttpNeuronForgeLocalAdapter {
    pub fn new(config: HttpNeuronForgeLocalAdapterConfig) -> Self {
        Self { config }
    }

    fn base_url(&self) -> String {
        self.config
            .base_url_override
            .clone()
            .unwrap_or_else(neuronforge_local_base_url)
    }
}

impl NeuronForgeTaskDeliveryAdapter for HttpNeuronForgeLocalAdapter {
    fn adapter_id(&self) -> &'static str {
        "http-neuronforge-local"
    }

    fn dispatch_task(
        &self,
        request: &NeuronForgeTaskDispatchRequest,
    ) -> NeuronForgeTaskDispatchResult {
        if request.task_id != ADMITTED_TASK_ID {
            return NeuronForgeTaskDispatchResult::DispatchUnavailable {
                summary: format!(
                    "task {:?} is not admitted for FA-Local dispatch (only {ADMITTED_TASK_ID:?} is)",
                    request.task_id
                ),
            };
        }

        let body = serde_json::json!({
            "contract_version": "FaLocalTaskDispatch.v1",
            "dispatch_id": request.dispatch_id,
            "request_id": request.request_id,
            "source_service_id": "fa-local",
            "destination_service_id": "neuronforge-local",
            "task_id": request.task_id,
            "scene_text": request.scene_text,
            "model_resource_disclosure": request.model_resource_disclosure,
            "operator_visible_message": request.operator_visible_message,
            "created_at": crate::domain::shared::now_utc(),
        });

        let url = format!(
            "{}{}",
            self.base_url(),
            NEURONFORGE_LOCAL_TASK_DISPATCH_PATH
        );
        let response = ureq::post(&url)
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
            .send_json(body);

        let receipt: Value = match response {
            Ok(resp) => match resp.into_json() {
                Ok(value) => value,
                Err(error) => {
                    return NeuronForgeTaskDispatchResult::DispatchUnavailable {
                        summary: format!(
                            "NeuronForge Local returned a response that could not be parsed as JSON: {error}"
                        ),
                    };
                }
            },
            Err(ureq::Error::Status(status, resp)) => {
                let body = resp
                    .into_string()
                    .unwrap_or_else(|_| "<unreadable response body>".to_owned());
                return NeuronForgeTaskDispatchResult::DispatchUnavailable {
                    summary: format!(
                        "NeuronForge Local rejected the dispatch (status {status}): {body}"
                    ),
                };
            }
            Err(ureq::Error::Transport(transport)) => {
                return NeuronForgeTaskDispatchResult::DispatchUnavailable {
                    summary: format!("could not reach NeuronForge Local at {url}: {transport}"),
                };
            }
        };

        match receipt
            .get("schema_validation_status")
            .and_then(Value::as_str)
        {
            Some("valid") => NeuronForgeTaskDispatchResult::Completed { receipt },
            Some("degraded") | Some("failed") => {
                NeuronForgeTaskDispatchResult::NotCompleted { receipt }
            }
            other => NeuronForgeTaskDispatchResult::DispatchUnavailable {
                summary: format!(
                    "NeuronForge Local receipt had an unrecognized or missing schema_validation_status: {other:?}"
                ),
            },
        }
    }
}
