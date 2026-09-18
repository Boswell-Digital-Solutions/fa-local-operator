use std::path::PathBuf;

use serde_json::Value;

use crate::adapters::execution_delivery::ExternalRouteDeliveryAdapter;
use crate::adapters::execution_delivery::local_file_write::{
    LocalFileWriteAdapterConfig, LocalFileWriteDeliveryAdapter,
};
use crate::adapters::execution_delivery::nmap_preflight::{
    NmapPreflightAdapterConfig, NmapPreflightDeliveryAdapter, NmapScanProfile,
};
use crate::adapters::execution_delivery::registry::AdapterRegistry;
use crate::adapters::exports::ForensicEventExportAdapter;
use crate::app::decision_service::DecisionService;
use crate::app::execution_service::{CoordinationContext, ExecutionService, ExecutionTrace};
use crate::app::forensic_service::{
    ForensicRecordContext, ForensicRecordInput, ForensicRecordKind, ForensicService,
};
use crate::app::routing_service::{RoutingInput, RoutingService};
use crate::domain::capabilities::CapabilityRegistryLoader;
use crate::domain::execution::{ExecutionPlan, ExecutionPlanValidator};
use crate::domain::forensics::{RedactionLevel, ValidatedForensicEvent};
use crate::domain::guards::DenialGuard;
use crate::domain::posture::RouteResolutionContext;
use crate::domain::routing::RouteDecision;
use crate::domain::shared::{ApprovalPosture, CapabilityId};
use crate::errors::{FaLocalError, FaLocalResult};

/// Which concrete external adapter to use for this run, and its config, with
/// the `supported_capability_id` left out: the capability the adapter must
/// serve is only known after the route decision resolves, so
/// [`ExecutionPipelineService::run`] fills it in from the resolved route.
#[derive(Debug, Clone)]
pub enum AdapterSelection {
    LocalFileWrite {
        delivery_root: PathBuf,
    },
    NmapPreflight {
        nmap_binary: PathBuf,
        scan_profile: NmapScanProfile,
    },
}

/// An [`AdapterSelection`] registered under an explicitly declared
/// capability, rather than the route's own top-level requested capability.
/// This is how a heterogeneous multi-capability plan gets more than one
/// adapter registered for [`DispatchMode::PerStep`] delivery: each entry's
/// `capability_id` should match a declared plan step's own `capability_id`,
/// not necessarily the route's.
#[derive(Debug, Clone)]
pub struct CapabilityScopedAdapterSelection {
    pub capability_id: CapabilityId,
    pub selection: AdapterSelection,
}

/// Which `ExecutionService` coordination call delivers an admitted route:
/// the whole plan in one call to a single adapter resolved for the route's
/// own top-level capability, or one call per declared step, each resolved
/// from the registry by that step's own capability. The configured
/// [`AdapterSelection`] (if any) is registered under the route's top-level
/// capability either way; in `PerStep` mode, a plan step declaring a
/// *different* capability then truthfully shows as unavailable rather than
/// silently succeeding, unless `additional_adapters` on [`ExecutionPipelineService::run`]
/// also registers an adapter for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DispatchMode {
    #[default]
    WholeRoute,
    PerStep,
}

#[derive(Debug, Clone)]
pub struct ExecutionPipelineInputs<'a> {
    pub request: &'a Value,
    pub requester_trust: &'a Value,
    pub policy: &'a Value,
    pub capability_registry: &'a Value,
    pub execution_plan: Option<&'a Value>,
}

#[derive(Debug, Clone)]
pub struct ForensicRecordOutcome {
    pub event: ValidatedForensicEvent,
    pub export_reference: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ExecutionPipelineOutcome {
    pub route_decision: RouteDecision,
    pub plan_denial: Option<DenialGuard>,
    pub execution_trace: Option<ExecutionTrace>,
    pub forensic_records: Vec<ForensicRecordOutcome>,
}

/// Composes [`DecisionService`], [`ExecutionPlanValidator`], [`RoutingService`],
/// [`ExecutionService`], and [`ForensicService`] into one bounded run: resolve
/// a route decision, and only when it admits execution, validate the declared
/// plan, dispatch through the adapter registry, and record forensic evidence
/// for every truthful outcome along the way (including denied, review-required,
/// and plan-invalid paths — never just the happy path).
#[derive(Debug, Default)]
pub struct ExecutionPipelineService;

impl ExecutionPipelineService {
    pub fn run(
        &self,
        inputs: ExecutionPipelineInputs<'_>,
        adapter_selection: Option<AdapterSelection>,
        additional_adapters: Vec<CapabilityScopedAdapterSelection>,
        dispatch_mode: DispatchMode,
        forensic_export_adapter: Option<&dyn ForensicEventExportAdapter>,
        context: RouteResolutionContext,
    ) -> FaLocalResult<ExecutionPipelineOutcome> {
        let route_decision = DecisionService.resolve_route_decision(
            inputs.request,
            inputs.requester_trust,
            inputs.policy,
            inputs.capability_registry,
            context.clone(),
        )?;

        let forensic_context = ForensicRecordContext::new(context.decided_at_utc);
        let mut forensic_records = Vec::new();

        match route_decision.resolved_approval_posture {
            ApprovalPosture::Denied => {
                forensic_records.push(self.record(
                    ForensicRecordKind::DenialIssued {
                        route_decision: route_decision.clone(),
                    },
                    RedactionLevel::SensitiveFieldsRedacted,
                    forensic_context,
                    forensic_export_adapter,
                )?);
                return Ok(ExecutionPipelineOutcome {
                    route_decision,
                    plan_denial: None,
                    execution_trace: None,
                    forensic_records,
                });
            }
            ApprovalPosture::ReviewRequired | ApprovalPosture::ExplicitOperatorApproval => {
                forensic_records.push(self.record(
                    ForensicRecordKind::RouteDecisionResolved {
                        route_decision: route_decision.clone(),
                    },
                    RedactionLevel::SensitiveFieldsRedacted,
                    forensic_context,
                    forensic_export_adapter,
                )?);
                return Ok(ExecutionPipelineOutcome {
                    route_decision,
                    plan_denial: None,
                    execution_trace: None,
                    forensic_records,
                });
            }
            ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed => {}
        }

        let plan_value = inputs.execution_plan.ok_or_else(|| {
            FaLocalError::ContractInvalid(
                "admitted route decision requires an execution plan".to_owned(),
            )
        })?;
        let capability_registry =
            CapabilityRegistryLoader::load_contract_value(inputs.capability_registry)?;
        let plan = ExecutionPlan::load_contract_value(plan_value)?;

        let validated_plan = match ExecutionPlanValidator::validate(&plan, &capability_registry) {
            Ok(validated_plan) => validated_plan,
            Err(denial) => {
                return Ok(ExecutionPipelineOutcome {
                    route_decision,
                    plan_denial: Some(denial),
                    execution_trace: None,
                    forensic_records,
                });
            }
        };

        let routing_input =
            RoutingInput::new(route_decision.clone(), Some(validated_plan.clone()))?;
        let selected_route = RoutingService.select_route(routing_input)?;

        let mut registry = AdapterRegistry::new();
        if let Some(selection) = adapter_selection {
            let route_capability_id = route_decision
                .capability_decision_summary
                .requested_capability_id;
            let adapter = build_adapter(selection, route_capability_id);
            registry.register(route_capability_id, adapter)?;
        }
        for entry in additional_adapters {
            let adapter = build_adapter(entry.selection, entry.capability_id);
            registry.register(entry.capability_id, adapter)?;
        }

        let coordination_context = CoordinationContext::new(
            context.decided_at_utc,
            context.decided_at_utc,
            context.decided_at_utc,
        );
        let execution_trace = match dispatch_mode {
            DispatchMode::WholeRoute => ExecutionService.deliver_selected_route_via_registry(
                &selected_route,
                &registry,
                coordination_context,
            )?,
            DispatchMode::PerStep => ExecutionService.deliver_plan_per_step_via_registry(
                &selected_route,
                &validated_plan,
                &registry,
                coordination_context,
            )?,
        };

        for status in &execution_trace.statuses {
            forensic_records.push(self.record(
                ForensicRecordKind::ExecutionStatusObserved {
                    route_decision: route_decision.clone(),
                    execution_status: status.clone(),
                },
                RedactionLevel::LinkageOnly,
                forensic_context,
                forensic_export_adapter,
            )?);
        }

        Ok(ExecutionPipelineOutcome {
            route_decision,
            plan_denial: None,
            execution_trace: Some(execution_trace),
            forensic_records,
        })
    }

    fn record(
        &self,
        kind: ForensicRecordKind,
        redaction_level: RedactionLevel,
        context: ForensicRecordContext,
        export_adapter: Option<&dyn ForensicEventExportAdapter>,
    ) -> FaLocalResult<ForensicRecordOutcome> {
        let input = ForensicRecordInput::new(kind, redaction_level, context)?;

        match export_adapter {
            Some(adapter) => {
                let exported = ForensicService.record_and_export_event_via(input, adapter)?;
                Ok(ForensicRecordOutcome {
                    event: exported.event,
                    export_reference: Some(exported.export_receipt.export_reference),
                })
            }
            None => {
                let event = ForensicService.record_event(input)?;
                Ok(ForensicRecordOutcome {
                    event,
                    export_reference: None,
                })
            }
        }
    }
}

fn build_adapter(
    selection: AdapterSelection,
    supported_capability_id: CapabilityId,
) -> Box<dyn ExternalRouteDeliveryAdapter> {
    match selection {
        AdapterSelection::LocalFileWrite { delivery_root } => {
            Box::new(LocalFileWriteDeliveryAdapter::new(
                LocalFileWriteAdapterConfig::new(supported_capability_id, delivery_root),
            )) as Box<dyn ExternalRouteDeliveryAdapter>
        }
        AdapterSelection::NmapPreflight {
            nmap_binary,
            scan_profile,
        } => Box::new(NmapPreflightDeliveryAdapter::new(
            NmapPreflightAdapterConfig::new(supported_capability_id, nmap_binary, scan_profile),
        )) as Box<dyn ExternalRouteDeliveryAdapter>,
    }
}
