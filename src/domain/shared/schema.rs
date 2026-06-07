use std::path::{Path, PathBuf};

use jsonschema::draft202012;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::errors::{FaLocalError, FaLocalResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaName {
    RequesterTrust,
    PolicyArtifact,
    CapabilityRegistry,
    ExecutionRequest,
    ExecutionPlan,
    ExecutionStatus,
    ReviewPackage,
    ForensicEvent,
    FrictionPayload,
    RouteDecision,
    DenialGuard,
    GnatDispatchEnvelope,
}

impl SchemaName {
    pub const ALL: [SchemaName; 12] = [
        SchemaName::RequesterTrust,
        SchemaName::PolicyArtifact,
        SchemaName::CapabilityRegistry,
        SchemaName::ExecutionRequest,
        SchemaName::ExecutionPlan,
        SchemaName::ExecutionStatus,
        SchemaName::ReviewPackage,
        SchemaName::ForensicEvent,
        SchemaName::FrictionPayload,
        SchemaName::RouteDecision,
        SchemaName::DenialGuard,
        SchemaName::GnatDispatchEnvelope,
    ];

    pub const fn all() -> &'static [SchemaName] {
        &Self::ALL
    }

    pub const fn fixture_prefix(self) -> &'static str {
        match self {
            SchemaName::RequesterTrust => "requester-trust",
            SchemaName::PolicyArtifact => "policy-artifact",
            SchemaName::CapabilityRegistry => "capability-registry",
            SchemaName::ExecutionRequest => "execution-request",
            SchemaName::ExecutionPlan => "execution-plan",
            SchemaName::ExecutionStatus => "execution-status",
            SchemaName::ReviewPackage => "review-package",
            SchemaName::ForensicEvent => "forensic-event",
            SchemaName::FrictionPayload => "friction-payload",
            SchemaName::RouteDecision => "route-decision",
            SchemaName::DenialGuard => "denial-guard",
            SchemaName::GnatDispatchEnvelope => "gnat-dispatch-envelope",
        }
    }

    pub const fn file_name(self) -> &'static str {
        match self {
            SchemaName::RequesterTrust => "requester-trust.schema.json",
            SchemaName::PolicyArtifact => "policy-artifact.schema.json",
            SchemaName::CapabilityRegistry => "capability-registry.schema.json",
            SchemaName::ExecutionRequest => "execution-request.schema.json",
            SchemaName::ExecutionPlan => "execution-plan.schema.json",
            SchemaName::ExecutionStatus => "execution-status.schema.json",
            SchemaName::ReviewPackage => "review-package.schema.json",
            SchemaName::ForensicEvent => "forensic-event.schema.json",
            SchemaName::FrictionPayload => "friction-payload.schema.json",
            SchemaName::RouteDecision => "route-decision.schema.json",
            SchemaName::DenialGuard => "denial-guard.schema.json",
            SchemaName::GnatDispatchEnvelope => "gnat-dispatch-envelope.schema.json",
        }
    }

    pub fn path(self) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("schemas")
            .join(self.file_name())
    }
}

pub fn load_json_value(path: impl AsRef<Path>) -> FaLocalResult<Value> {
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn load_contract_from_path<T: DeserializeOwned>(
    schema_name: SchemaName,
    path: impl AsRef<Path>,
) -> FaLocalResult<T> {
    let value = load_json_value(path)?;
    deserialize_contract_value(schema_name, &value)
}

pub fn validate_contract_value(schema_name: SchemaName, value: &Value) -> FaLocalResult<()> {
    let schema = load_json_value(schema_name.path())?;
    let validator = draft202012::options()
        .should_validate_formats(true)
        .build(&schema)
        .map_err(|error| FaLocalError::SchemaCompile {
            schema: schema_name.file_name().to_owned(),
            message: error.to_string(),
        })?;

    let errors = validator
        .iter_errors(value)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(FaLocalError::SchemaValidation {
            schema: schema_name.file_name().to_owned(),
            errors,
        })
    }
}

pub fn deserialize_contract_value<T: DeserializeOwned>(
    schema_name: SchemaName,
    value: &Value,
) -> FaLocalResult<T> {
    validate_contract_value(schema_name, value)?;
    Ok(serde_json::from_value(value.clone())?)
}
