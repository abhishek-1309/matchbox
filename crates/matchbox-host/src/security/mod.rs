use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SecurityError {
    #[error("path traversal attempt detected")]
    PathTraversalAttempt,
    #[error("path outside jail root")]
    OutsideJail,
    #[error("invalid path encoding")]
    InvalidPath,
}

/// Resolves a guest-provided path to a sandboxed host path within `jail_root`.
/// All guest paths are treated as relative to the jail root.
pub fn resolve_sandboxed_path(jail_root: &Path, guest_path_str: &str) -> Result<PathBuf, SecurityError> {
    // Strip leading root to prevent absolute path escapes
    let stripped = guest_path_str.trim_start_matches('/');

    // Reject any component that tries `..`
    for component in Path::new(stripped).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(SecurityError::PathTraversalAttempt);
        }
        if matches!(component, std::path::Component::RootDir) {
            return Err(SecurityError::PathTraversalAttempt);
        }
    }

    let target = jail_root.join(stripped);

    // Canonicalize and verify it's still under jail root
    let canonical = target
        .canonicalize()
        .map_err(|_| SecurityError::InvalidPath)?;

    if !canonical.starts_with(jail_root) {
        return Err(SecurityError::OutsideJail);
    }

    Ok(canonical)
}