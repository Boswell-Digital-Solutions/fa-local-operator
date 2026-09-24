//! Read-only HTTP serving surface for FA Local's capability registry.
//!
//! Authorized by `BDS-FAL-DAEMON-v0.1`'s implementation scoping packet
//! (`docs/plans/active/BDS_FAL_DAEMON_v0.1/02_IMPLEMENTATION_SCOPING_PACKET.md`).
//! This is the crate's first HTTP-serving surface -- everything else FA
//! Local does over HTTP is as a *client* (`integrations::df_local`,
//! `integrations::neuronforge_local`), which the ratifying proposal
//! (`01_CURRENT_STATE_DOCTRINE_AND_PROPOSAL.md`) found does not conflict
//! with FA Local's prior "no HTTP surface" doctrine, since that doctrine
//! constrains what it *serves*, not what it calls.
//!
//! Exposes exactly one route, `GET /api/v1/capabilities/{capability_id}`,
//! default-off behind `FA_LOCAL_SERVE_ENABLED` (checked by
//! `src/bin/fa_local_run.rs` before any of this module's code ever runs).
//! Read-only: no route here ever admits or dispatches execution, and
//! nothing here calls `domain::capabilities::CapabilityRegistryLoader::admit_execution_request`.

pub mod http_server;
pub mod token_verify;

pub use http_server::{
    DEFAULT_PUBLIC_KEYS_ENV, DEFAULT_SERVE_PORT, load_public_keys_from_env, run_server,
};
pub use token_verify::{AuthDenyReason, REQUIRED_SCOPE, verify_bearer_token};
