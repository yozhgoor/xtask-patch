use thiserror::Error;

mod manifest;
mod patches;

pub use manifest::Manifest;
pub use patches::{Patch, Patches};

#[derive(Debug, Error)]
pub enum Error {
    #[error("package not found: {package}")]
    NotFound { package: String },
    #[error("package appears in multiple sources: {package} ({sources:?})")]
    Multiple {
        package: String,
        sources: Vec<String>,
    },
    #[error("package already exists in source {src}: {package}")]
    Exists { package: String, src: String },
    #[error("failed to parse patch: {line}")]
    Parse { line: String },
    #[error("failed to parse project's metadata: {source}")]
    ParseMetadata { source: cargo_metadata::Error },
    #[error("failed to read manifest at {path}: {source}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write manifest at {path}: {source}")]
    Write {
        path: String,
        #[source]
        source: std::io::Error,
    },
}
