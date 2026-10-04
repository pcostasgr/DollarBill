use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildInfo {
    pub package_version: String,
    pub git_commit: String,
    pub source_sha256: String,
    pub rustc: String,
    pub target: String,
}

impl BuildInfo {
    pub fn current() -> Self {
        Self {
            package_version: env!("CARGO_PKG_VERSION").into(),
            git_commit: env!("DOLLARBILL_GIT_COMMIT").into(),
            source_sha256: env!("DOLLARBILL_SOURCE_SHA256").into(),
            rustc: env!("DOLLARBILL_RUSTC").into(),
            target: env!("DOLLARBILL_TARGET").into(),
        }
    }
}
