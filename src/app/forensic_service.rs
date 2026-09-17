use crate::adapters::exports::{
    ForensicEventExportAdapter, ForensicExportReceipt, ForensicExportResult,
};
use crate::domain::forensics::{
    ForensicEvent, ForensicEventType, RedactionLevel, ValidatedForensicEvent,
};
use crate::domain::review::ValidatedReviewPackage;
use crate::domain::routing::RouteDecision;
use crate::domain::shared::{
    ApprovalPosture, DegradedSubtype, ExecutionState, ForensicEventId, TimestampUtc, now_utc,
};
use crate::domain::status::ValidatedExecutionStatus;
use crate::errors::{FaLocalError, FaLocalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForensicRecordContext {
    pub recorded_at_utc: TimestampUtc,
}

impl ForensicRecordContext {
    pub fn new(recorded_at_utc: TimestampUtc) -> Self {
        Self { recorded_at_utc }
    }
}

impl Default for ForensicRecordContext {
    fn default() -> Self {
        Self::new(now_utc())
    }
}

#[derive(Debug, Clone)]
pub enum ForensicRecordKind {
    DenialIssued {
        route_decision: RouteDecision,
    },
    RouteDecisionResolved {
        route_decision: RouteDecision,
    },
    ReviewPackagePrepared {
        route_decision: RouteDecision,
        review_package: ValidatedReviewPackage,
    },
    ExecutionStatusObserved {
        route_decision: RouteDecision,
        execution_status: ValidatedExecutionStatus,
    },
}

#[derive(Debug, Clone)]
pub struct ForensicRecordInput {
    pub kind: ForensicRecordKind,
    pub redaction_level: RedactionLevel,
    pub context: ForensicRecordContext,
}

impl ForensicRecordInput {
    pub fn new(
        kind: ForensicRecordKind,
        redaction_level: RedactionLevel,
        context: ForensicRecordContext,
    ) -> FaLocalResult<Self> {
        validate_record_kind(&kind)?;
        Ok(Self {
            kind,
            redaction_level,
            context,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedForensicRecord {
    pub event: ValidatedForensicEvent,
    pub export_receipt: ForensicExportReceipt,
}

#[derive(Debug, Default)]
pub struct ForensicService;

impl ForensicService {
    pub fn record_event(
        &self,
        input: ForensicRecordInput,
    ) -> FaLocalResult<ValidatedForensicEvent> {
        let record = build_forensic_record(&input.kind)?;
        validate_summary_truthfulness(
            &record.summary,
            record.execution_state,
            record.degraded_subtype,
        )?;

        let event = ForensicEvent::new(
            ForensicEventId::new(),
            record.correlation_id,
            record.request_id,
            record.event_type,
            record.route_decision_id,
            record.execution_plan_id,
            record.stable_plan_hash,
            record.review_package_id,
            input.context.recorded_at_utc,
            record.current_posture,
            record.execution_state,
            record.degraded_subtype,
            record.summary,
            input.redaction_level,
            true,
        )?;

        ValidatedForensicEvent::new(event)
    }

    pub fn record_and_export_event<A: ForensicEventExportAdapter>(
        &self,
        input: ForensicRecordInput,
        adapter: &A,
    ) -> FaLocalResult<ExportedForensicRecord> {
        self.record_and_export_event_via(input, adapter)
    }

    /// Same as [`Self::record_and_export_event`], but takes the adapter as a
    /// trait object so a caller that only learns which concrete adapter to
    /// use at runtime (e.g. from CLI flags) does not need a generic type
    /// parameter to call it.
    pub fn record_and_export_event_via(
        &self,
        input: ForensicRecordInput,
        adapter: &dyn ForensicEventExportAdapter,
    ) -> FaLocalResult<ExportedForensicRecord> {
        let event = self.record_event(input)?;
        let export_receipt = map_export_result(adapter.adapter_id(), adapter.export_event(&event))?;

        Ok(ExportedForensicRecord {
            event,
            export_receipt,
        })
    }
}

#[derive(Debug)]
struct ForensicRecord {
    correlation_id: crate::CorrelationId,
    request_id: crate::RequestId,
    event_type: ForensicEventType,
    route_decision_id: Option<crate::RouteDecisionId>,
    execution_plan_id: Option<crate::ExecutionPlanId>,
    stable_plan_hash: Option<String>,
    review_package_id: Option<crate::ReviewPackageId>,
    current_posture: ApprovalPosture,
    execution_state: ExecutionState,
    degraded_subtype: Option<DegradedSubtype>,
    summary: String,
}

fn build_forensic_record(kind: &ForensicRecordKind) -> FaLocalResult<ForensicRecord> {
    match kind {
        ForensicRecordKind::DenialIssued { route_decision } => Ok(ForensicRecord {
            correlation_id: route_decision.correlation_id,
            request_id: route_decision.request_id,
            event_type: ForensicEventType::DenialIssued,
            route_decision_id: None,
            execution_plan_id: None,
            stable_plan_hash: None,
            review_package_id: None,
            current_posture: ApprovalPosture::Denied,
            execution_state: ExecutionState::Denied,
            degraded_subtype: None,
            summary: route_decision.operator_visible_summary.clone(),
        }),
        ForensicRecordKind::RouteDecisionResolved { route_decision } => Ok(ForensicRecord {
            correlation_id: route_decision.correlation_id,
            request_id: route_decision.request_id,
            event_type: ForensicEventType::RouteDecisionResolved,
            route_decision_id: Some(route_decision.route_decision_id),
            execution_plan_id: None,
            stable_plan_hash: None,
            review_package_id: None,
            current_posture: route_decision.resolved_approval_posture,
            execution_state: pre_execution_state_for(route_decision.resolved_approval_posture)?,
            degraded_subtype: None,
            summary: route_decision.operator_visible_summary.clone(),
        }),
        ForensicRecordKind::ReviewPackagePrepared {
            route_decision,
            review_package,
        } => Ok(ForensicRecord {
            correlation_id: route_decision.correlation_id,
            request_id: route_decision.request_id,
            event_type: ForensicEventType::ReviewPackagePrepared,
            route_decision_id: Some(route_decision.route_decision_id),
            execution_plan_id: review_package.package.execution_plan_id,
            stable_plan_hash: review_package.package.stable_plan_hash.clone(),
            review_package_id: Some(review_package.package.review_package_id),
            current_posture: review_package.package.current_posture,
            execution_state: review_package
                .package
                .execution_status_context
                .as_ref()
                .map(|context| context.state)
                .unwrap_or(ExecutionState::WaitingExplicitApproval),
            degraded_subtype: review_package.package.degraded_or_fallback_posture,
            summary: review_package_summary(review_package),
        }),
        ForensicRecordKind::ExecutionStatusObserved {
            route_decision,
            execution_status,
        } => Ok(ForensicRecord {
            correlation_id: route_decision.correlation_id,
            request_id: route_decision.request_id,
            event_type: ForensicEventType::ExecutionStatusObserved,
            route_decision_id: Some(route_decision.route_decision_id),
            execution_plan_id: execution_status.status.execution_plan_id,
            stable_plan_hash: execution_status.status.stable_plan_hash.clone(),
            review_package_id: None,
            current_posture: execution_status.status.current_posture,
            execution_state: execution_status.status.state,
            degraded_subtype: execution_status.status.degraded_subtype,
            summary: execution_status
                .status
                .truthful_user_visible_summary
                .clone(),
        }),
    }
}

fn review_package_summary(review_package: &ValidatedReviewPackage) -> String {
    if is_explicit_fallback_subtype(review_package.package.degraded_or_fallback_posture) {
        "bounded review handoff prepared with explicit fallback posture".to_owned()
    } else {
        "bounded review handoff prepared for explicit operator approval".to_owned()
    }
}

fn validate_record_kind(kind: &ForensicRecordKind) -> FaLocalResult<()> {
    match kind {
        ForensicRecordKind::DenialIssued { route_decision } => {
            validate_route_decision_surface(route_decision)?;
            require_posture(
                route_decision.resolved_approval_posture,
                ApprovalPosture::Denied,
                "forensic denial record requires denied route posture",
            )
        }
        ForensicRecordKind::RouteDecisionResolved { route_decision } => {
            validate_route_decision_surface(route_decision)
        }
        ForensicRecordKind::ReviewPackagePrepared {
            route_decision,
            review_package,
        } => {
            validate_route_decision_surface(route_decision)?;
            review_package.package.validate()?;

            if route_decision.resolved_approval_posture != ApprovalPosture::ExplicitOperatorApproval
            {
                return Err(contract_invalid(
                    "forensic review-package record currently requires explicit_operator_approval posture",
                ));
            }

            if review_package.package.current_posture != ApprovalPosture::ExplicitOperatorApproval {
                return Err(contract_invalid(
                    "forensic review-package record currently requires explicit_operator_approval review package posture",
                ));
            }

            if review_package.package.originating_request_id != route_decision.request_id {
                return Err(contract_invalid(
                    "forensic review-package record request_id does not match route decision",
                ));
            }

            if review_package.package.correlation_id != route_decision.correlation_id {
                return Err(contract_invalid(
                    "forensic review-package record correlation_id does not match route decision",
                ));
            }

            if review_package.package.route_decision_id != route_decision.route_decision_id {
                return Err(contract_invalid(
                    "forensic review-package record route_decision_id does not match route decision",
                ));
            }

            Ok(())
        }
        ForensicRecordKind::ExecutionStatusObserved {
            route_decision,
            execution_status,
        } => {
            validate_route_decision_surface(route_decision)?;
            execution_status.status.validate()?;

            if !matches!(
                route_decision.resolved_approval_posture,
                ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed
            ) {
                return Err(contract_invalid(
                    "forensic execution-status record requires an admitted route posture",
                ));
            }

            if execution_status.status.request_id != route_decision.request_id {
                return Err(contract_invalid(
                    "forensic execution-status record request_id does not match route decision",
                ));
            }

            if execution_status.status.correlation_id != route_decision.correlation_id {
                return Err(contract_invalid(
                    "forensic execution-status record correlation_id does not match route decision",
                ));
            }

            if execution_status.status.current_posture != route_decision.resolved_approval_posture {
                return Err(contract_invalid(
                    "forensic execution-status record posture does not match route decision",
                ));
            }

            if !matches!(
                execution_status.status.state,
                ExecutionState::AdmittedNotStarted
                    | ExecutionState::InProgress
                    | ExecutionState::Degraded
                    | ExecutionState::PartialSuccess
                    | ExecutionState::CompletedWithConstraints
                    | ExecutionState::Completed
                    | ExecutionState::Failed
                    | ExecutionState::Canceled
            ) {
                return Err(contract_invalid(
                    "forensic execution-status record must use an admitted execution_state",
                ));
            }

            Ok(())
        }
    }
}

fn map_export_result(
    adapter_id: &str,
    export_result: ForensicExportResult,
) -> FaLocalResult<ForensicExportReceipt> {
    validate_adapter_id(adapter_id)?;

    match export_result {
        ForensicExportResult::Exported { export_reference } => {
            validate_export_reference(&export_reference)?;
            Ok(ForensicExportReceipt {
                adapter_id: adapter_id.to_owned(),
                export_reference,
            })
        }
        ForensicExportResult::DependencyUnavailable { summary } => {
            validate_bounded_detail(&summary, "forensic export dependency summary")?;
            Err(contract_invalid(format!(
                "forensic export dependency unavailable: {summary}"
            )))
        }
        ForensicExportResult::Unsupported { summary } => {
            validate_bounded_detail(&summary, "forensic export unsupported summary")?;
            Err(contract_invalid(format!(
                "forensic export unsupported: {summary}"
            )))
        }
    }
}

fn pre_execution_state_for(posture: ApprovalPosture) -> FaLocalResult<ExecutionState> {
    match posture {
        ApprovalPosture::Denied => Ok(ExecutionState::Denied),
        ApprovalPosture::ReviewRequired => Ok(ExecutionState::ReviewRequired),
        ApprovalPosture::ExplicitOperatorApproval => Ok(ExecutionState::WaitingExplicitApproval),
        ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed => {
            Ok(ExecutionState::AdmittedNotStarted)
        }
    }
}

fn validate_route_decision_surface(route_decision: &RouteDecision) -> FaLocalResult<()> {
    match route_decision.resolved_approval_posture {
        ApprovalPosture::Denied => {
            if route_decision.execution_allowed
                || route_decision.review_required
                || route_decision.explicit_approval_required
            {
                return Err(contract_invalid(
                    "denied route decision is inconsistent with forensic recorder expectations",
                ));
            }
        }
        ApprovalPosture::ReviewRequired => {
            if route_decision.execution_allowed
                || !route_decision.review_required
                || route_decision.explicit_approval_required
            {
                return Err(contract_invalid(
                    "review_required route decision is inconsistent with forensic recorder expectations",
                ));
            }
        }
        ApprovalPosture::ExplicitOperatorApproval => {
            if route_decision.execution_allowed
                || route_decision.review_required
                || !route_decision.explicit_approval_required
            {
                return Err(contract_invalid(
                    "explicit_operator_approval route decision is inconsistent with forensic recorder expectations",
                ));
            }
        }
        ApprovalPosture::PolicyPreapproved | ApprovalPosture::ExecuteAllowed => {
            if !route_decision.execution_allowed
                || route_decision.review_required
                || route_decision.explicit_approval_required
            {
                return Err(contract_invalid(
                    "admitted route decision is inconsistent with forensic recorder expectations",
                ));
            }
        }
    }

    Ok(())
}

fn validate_summary_truthfulness(
    summary: &str,
    execution_state: ExecutionState,
    degraded_subtype: Option<DegradedSubtype>,
) -> FaLocalResult<()> {
    validate_bounded_detail(summary, "forensic event summary")?;

    if mentions_success_or_completion(summary)
        && !matches!(
            execution_state,
            ExecutionState::Completed
                | ExecutionState::CompletedWithConstraints
                | ExecutionState::PartialSuccess
        )
    {
        return Err(contract_invalid(
            "forensic event summary must not claim success or completion before truthful execution completion",
        ));
    }

    if mentions_failure(summary) && execution_state != ExecutionState::Failed {
        return Err(contract_invalid(
            "forensic event summary must not claim failure outside failed execution_state",
        ));
    }

    if mentions_cancellation(summary) && execution_state != ExecutionState::Canceled {
        return Err(contract_invalid(
            "forensic event summary must not claim cancellation outside canceled execution_state",
        ));
    }

    if mentions_degraded_or_constraints(summary) && degraded_subtype.is_none() {
        return Err(contract_invalid(
            "forensic event summary must not claim degraded or constrained execution without explicit degraded_subtype",
        ));
    }

    Ok(())
}

fn validate_adapter_id(adapter_id: &str) -> FaLocalResult<()> {
    if adapter_id.is_empty() || adapter_id.len() > 64 {
        return Err(contract_invalid(
            "forensic export adapter_id must be between 1 and 64 characters",
        ));
    }
    Ok(())
}

fn validate_export_reference(export_reference: &str) -> FaLocalResult<()> {
    if export_reference.is_empty() || export_reference.len() > 160 {
        return Err(contract_invalid(
            "forensic export adapter must return a bounded export_reference between 1 and 160 characters",
        ));
    }
    Ok(())
}

fn validate_bounded_detail(value: &str, field_name: &str) -> FaLocalResult<()> {
    if value.is_empty() || value.len() > 160 {
        return Err(contract_invalid(format!(
            "{field_name} must be between 1 and 160 characters",
        )));
    }
    Ok(())
}

fn require_posture(
    posture: ApprovalPosture,
    expected: ApprovalPosture,
    message: &'static str,
) -> FaLocalResult<()> {
    if posture == expected {
        Ok(())
    } else {
        Err(contract_invalid(message))
    }
}

fn is_explicit_fallback_subtype(value: Option<DegradedSubtype>) -> bool {
    matches!(
        value,
        Some(
            DegradedSubtype::DegradedFallbackEquivalent | DegradedSubtype::DegradedFallbackLimited
        )
    )
}

fn mentions_success_or_completion(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("success") || lower.contains("succeeded") || lower.contains("completed")
}

fn mentions_failure(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("failed") || lower.contains("failure")
}

fn mentions_cancellation(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("canceled") || lower.contains("cancelled")
}

fn mentions_degraded_or_constraints(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.contains("degraded")
        || lower.contains("constraint")
        || lower.contains("constraints")
        || lower.contains("limited")
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}
