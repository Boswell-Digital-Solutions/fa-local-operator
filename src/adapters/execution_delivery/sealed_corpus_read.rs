use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::domain::reuse_reconnaissance::CorpusDocument;
use crate::errors::{FaLocalError, FaLocalResult};

const MAX_MEMBER_COUNT: usize = 64;
const MAX_MEMBER_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCorpusReadRequest {
    pub allowed_root: PathBuf,
    pub manifest_relative_path: String,
    pub max_total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedCorpusReadReceipt {
    pub corpus_id: String,
    pub aggregate_sha256: String,
    pub member_count: usize,
    pub total_bytes: u64,
    pub documents: Vec<CorpusDocument>,
}

#[derive(Debug, Default)]
pub struct SealedCorpusReadAdapter;

impl SealedCorpusReadAdapter {
    pub fn read_candidate_visible(
        &self,
        request: &SealedCorpusReadRequest,
    ) -> FaLocalResult<SealedCorpusReadReceipt> {
        let root = canonical_directory(&request.allowed_root)?;
        let manifest_relative = validate_relative_path(&request.manifest_relative_path)?;
        let manifest_path = confined_regular_file(&root, &manifest_relative)?;
        let manifest_raw = std::fs::read_to_string(&manifest_path)?;
        let manifest: SealedCorpusManifest = serde_json::from_str(&manifest_raw)?;

        if manifest.schema_version != "FraaSealedCorpusManifest.v0"
            || manifest.exposure_class != "candidate_visible"
            || manifest.aggregate_profile != "path-sha256-size-v1"
        {
            return Err(contract_invalid(
                "sealed corpus manifest identity or exposure class is not admitted",
            ));
        }
        if manifest.members.is_empty() || manifest.members.len() > MAX_MEMBER_COUNT {
            return Err(contract_invalid(
                "sealed corpus manifest member count is outside admitted bounds",
            ));
        }

        let mut members = manifest.members;
        members.sort_by(|left, right| left.path.cmp(&right.path));
        let mut seen = BTreeSet::new();
        let mut documents = Vec::with_capacity(members.len());
        let mut total_bytes = 0_u64;
        let mut aggregate_preimage = Vec::new();

        for member in members {
            if !seen.insert(member.path.clone()) {
                return Err(contract_invalid(
                    "sealed corpus manifest contains duplicate member paths",
                ));
            }
            let relative = validate_relative_path(&member.path)?;
            reject_oracle_component(&relative)?;
            let path = confined_regular_file(&root, &relative)?;
            let metadata = std::fs::metadata(&path)?;
            if metadata.len() != member.size_bytes || metadata.len() > MAX_MEMBER_BYTES {
                return Err(contract_invalid(format!(
                    "sealed corpus member {} size does not match admitted manifest",
                    member.path
                )));
            }
            total_bytes = total_bytes
                .checked_add(metadata.len())
                .ok_or_else(|| contract_invalid("sealed corpus byte count overflowed"))?;
            if total_bytes > request.max_total_bytes {
                return Err(contract_invalid(
                    "sealed corpus exceeds request max_total_bytes",
                ));
            }

            let bytes = std::fs::read(&path)?;
            let observed_sha256 = sha256_hex(&bytes);
            if observed_sha256 != member.sha256 {
                return Err(contract_invalid(format!(
                    "sealed corpus member {} digest mismatch",
                    member.path
                )));
            }
            let content = String::from_utf8(bytes).map_err(|_| {
                contract_invalid(format!(
                    "sealed corpus member {} is not UTF-8 text",
                    member.path
                ))
            })?;

            aggregate_preimage.extend_from_slice(member.path.as_bytes());
            aggregate_preimage.push(0);
            aggregate_preimage.extend_from_slice(member.sha256.as_bytes());
            aggregate_preimage.push(0);
            aggregate_preimage.extend_from_slice(member.size_bytes.to_string().as_bytes());
            aggregate_preimage.push(b'\n');

            documents.push(CorpusDocument {
                relative_path: member.path,
                sha256: member.sha256,
                size_bytes: member.size_bytes,
                content,
            });
        }

        Ok(SealedCorpusReadReceipt {
            corpus_id: manifest.corpus_id,
            aggregate_sha256: sha256_hex(&aggregate_preimage),
            member_count: documents.len(),
            total_bytes,
            documents,
        })
    }
}

#[derive(Debug, Deserialize)]
struct SealedCorpusManifest {
    schema_version: String,
    corpus_id: String,
    exposure_class: String,
    aggregate_profile: String,
    members: Vec<SealedCorpusMember>,
}

#[derive(Debug, Deserialize)]
struct SealedCorpusMember {
    path: String,
    sha256: String,
    size_bytes: u64,
}

fn canonical_directory(path: &Path) -> FaLocalResult<PathBuf> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(contract_invalid(
            "sealed corpus root must be a real directory, not a symlink",
        ));
    }
    Ok(std::fs::canonicalize(path)?)
}

fn confined_regular_file(root: &Path, relative: &Path) -> FaLocalResult<PathBuf> {
    let candidate = root.join(relative);
    let metadata = std::fs::symlink_metadata(&candidate)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(contract_invalid(
            "sealed corpus member must be a regular non-symlink file",
        ));
    }
    let canonical = std::fs::canonicalize(candidate)?;
    if !canonical.starts_with(root) {
        return Err(contract_invalid(
            "sealed corpus member escapes the admitted root",
        ));
    }
    Ok(canonical)
}

fn validate_relative_path(raw: &str) -> FaLocalResult<PathBuf> {
    if raw.is_empty() || raw.contains('\\') || raw.contains(':') {
        return Err(contract_invalid(
            "sealed corpus paths must be nonempty repository-style relative paths",
        ));
    }
    let path = Path::new(raw);
    if path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(contract_invalid(
            "sealed corpus path contains an absolute, parent, or non-normal component",
        ));
    }
    Ok(path.to_path_buf())
}

fn reject_oracle_component(path: &Path) -> FaLocalResult<()> {
    for component in path.components() {
        let Component::Normal(value) = component else {
            return Err(contract_invalid("sealed corpus path is not normalized"));
        };
        let label = value.to_string_lossy().to_ascii_lowercase();
        if label == "oracle" || label == "hidden_oracle" || label == "hidden-oracle" {
            return Err(contract_invalid(
                "candidate-visible corpus manifest may not reference hidden oracle material",
            ));
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        write!(&mut out, "{byte:02x}").expect("writing SHA-256 digest must succeed");
    }
    out
}

fn contract_invalid(message: impl Into<String>) -> FaLocalError {
    FaLocalError::ContractInvalid(message.into())
}
