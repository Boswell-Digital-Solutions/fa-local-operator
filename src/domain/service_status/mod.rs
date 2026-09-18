//! FA Local's own service-status truth, projected onto
//! `forge-local-systems-runtime`'s canonical `service-status.schema.json`
//! (vendored at `schemas/forge_local_runtime/`), for Forge_Command's
//! FC-LTA-P007 (`runtime-envelope-schema-valid`).
//!
//! FLO has no existing whole-service status computation to project from --
//! its only status concept, [`crate::domain::status::ExecutionStatus`], is
//! per-request, not per-service. [`operational_facts`] is the single source
//! of truth this module and the `status` CLI subcommand both read, so the
//! two real structural facts it reports are never hardcoded a second time.
//!
//! The canonical schema is a different, external contract from FLO's own
//! `schemas/` family (`additionalProperties: false`, its own narrower
//! shape). It is vendored separately at `schemas/forge_local_runtime/`
//! rather than added to [`crate::domain::shared::SchemaName`], the same
//! separation Cortex and DataForge Local use for this same schema pair.
//!
//! FLO is a CLI with no HTTP surface (see the repo `CLAUDE.md`).
//! Forge_Command reads this projection by spawning the compiled
//! `fa-local-run` binary's `canonical-status` subcommand as a subprocess and
//! parsing its one line of stdout JSON.

use std::path::{Path, PathBuf};

use jsonschema::{Resource, draft202012};
use serde_json::{Value, json};

use crate::config::SERVICE_ID;
use crate::domain::shared::{DegradedSubtype, now_utc};
use crate::errors::{FaLocalError, FaLocalResult};

const SERVICE_CLASS: &str = "execution";
const CANONICAL_SCHEMA_FILE_NAME: &str = "forge_local_runtime/service-status.schema.json";

// `service-status.schema.json` `$ref`s this by relative path
// (`./denial-state.schema.json`, resolved against its own `$id`). The
// `jsonschema` crate resolves every `$ref` eagerly at `build()` time (unlike
// Python's `jsonschema`, which resolves lazily only on the branch actually
// validated), and this crate has no `resolve-http` feature, so the resource
// must be registered in-memory under this exact URI rather than fetched.
const DENIAL_STATE_SCHEMA_URI: &str =
    "https://boswelldigitalsolutions.com/schemas/forge-local-runtime/denial-state.schema.json";

fn vendored_schema_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join("forge_local_runtime")
}

fn canonical_schema_path() -> PathBuf {
    vendored_schema_dir().join("service-status.schema.json")
}

fn denial_state_schema_path() -> PathBuf {
    vendored_schema_dir().join("denial-state.schema.json")
}

/// The real, structural facts about FA Local's own operational surface.
///
/// Both are structural facts about *code presence*, not runtime probes:
/// `execution_enabled` is `true` because `fa-local-run execute` exists and
/// dispatches admitted plans through the `AdapterRegistry`
/// (`src/bin/fa_local_run.rs`); `writeback_wired` stays `false` because
/// `DfLocalAdapter::post_execution_status_event`
/// (`src/integrations/df_local/mod.rs`) unconditionally returns
/// `FaLocalError::WritebackNotWired` until DataForge Local's Phase X4
/// endpoint exists. Read this function rather than re-stating these
/// booleans anywhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaLocalOperationalFacts {
    pub execution_enabled: bool,
    pub writeback_wired: bool,
}

pub fn operational_facts() -> FaLocalOperationalFacts {
    FaLocalOperationalFacts {
        execution_enabled: true,
        writeback_wired: false,
    }
}

/// Project FA Local's own real operational facts onto the canonical
/// `service-status.schema.json` envelope.
///
/// Never invents a signal: the only two real facts FA Local can currently
/// report about itself are `execution_enabled: true` and
/// `writeback_wired: false`, so this reports the one honest state that
/// follows from them (`degraded` / `unavailable_dependency_block` -- core
/// validation and dispatch work, but forensic status events cannot be
/// staged to DataForge Local because its Phase X4 endpoint does not exist
/// yet). It never carries forward the old `status` output's fabricated
/// `posture: "policy_first_admission"` string, which was not derived from
/// any real check.
pub fn build_canonical_service_status_envelope() -> FaLocalResult<Value> {
    let facts = operational_facts();

    if !facts.execution_enabled || facts.writeback_wired {
        // Not reachable today -- operational_facts() reports
        // execution_enabled: true, writeback_wired: false -- but if either
        // ever flips again, fail loudly rather than silently reporting this
        // envelope as if nothing changed.
        return Err(FaLocalError::InternalInvariant(
            "operational_facts() reported a combination \
             build_canonical_service_status_envelope has no honest envelope for yet -- \
             update this projection before shipping the change that flipped it."
                .to_string(),
        ));
    }

    let state = "degraded";
    let message = "FA Local validates and dispatches execution requests, but DataForge Local \
         writeback has no endpoint yet: forensic status events are recorded locally and not \
         staged to DataForge Local.";
    let degraded_subtype = serde_json::to_value(DegradedSubtype::UnavailableDependencyBlock)?;

    let envelope = json!({
        "service_id": SERVICE_ID,
        "service_class": SERVICE_CLASS,
        "state": state,
        "degraded_subtype": degraded_subtype,
        "operator_visible_message": message,
        "readiness_summary": {
            "readiness_class": state,
            "summary": message,
        },
        "last_updated_at": now_utc().to_rfc3339(),
    });

    let schema = load_canonical_schema()?;
    let denial_state_schema = load_json_file(&denial_state_schema_path())?;
    let validator = draft202012::options()
        .should_validate_formats(true)
        .with_resource(
            DENIAL_STATE_SCHEMA_URI,
            Resource::from_contents(denial_state_schema),
        )
        .build(&schema)
        .map_err(|error| FaLocalError::SchemaCompile {
            schema: CANONICAL_SCHEMA_FILE_NAME.to_owned(),
            message: error.to_string(),
        })?;

    let errors: Vec<String> = validator
        .iter_errors(&envelope)
        .map(|error| error.to_string())
        .collect();
    if !errors.is_empty() {
        return Err(FaLocalError::SchemaValidation {
            schema: CANONICAL_SCHEMA_FILE_NAME.to_owned(),
            errors,
        });
    }

    Ok(envelope)
}

fn load_canonical_schema() -> FaLocalResult<Value> {
    load_json_file(&canonical_schema_path())
}

fn load_json_file(path: &Path) -> FaLocalResult<Value> {
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_is_canonical_schema_valid() {
        let envelope = build_canonical_service_status_envelope().expect("envelope should build");
        assert_eq!(envelope["service_id"], "fa-local");
        assert_eq!(envelope["service_class"], "execution");
        assert_eq!(envelope["state"], "degraded");
        assert_eq!(envelope["degraded_subtype"], "unavailable_dependency_block");
    }

    #[test]
    fn envelope_never_carries_the_fabricated_posture_field() {
        let envelope = build_canonical_service_status_envelope().expect("envelope should build");
        let obj = envelope.as_object().expect("envelope is an object");
        assert!(
            !obj.contains_key("posture"),
            "posture was never a real signal -- it must not appear in the canonical envelope"
        );
    }

    #[test]
    fn envelope_reflects_the_real_operational_facts_in_its_message() {
        let facts = operational_facts();
        assert!(facts.execution_enabled);
        assert!(!facts.writeback_wired);

        let envelope = build_canonical_service_status_envelope().expect("envelope should build");
        let message = envelope["operator_visible_message"]
            .as_str()
            .expect("message is a string");
        assert!(message.contains("DataForge Local"));
    }
}
