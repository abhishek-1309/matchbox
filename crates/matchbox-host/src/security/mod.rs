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

pub fn resolve_sandboxed_path(jail_root: &Path, guest_path_str: &str) -> Result<PathBuf, SecurityError> {
    let stripped = guest_path_str.trim_start_matches('/');
    for component in Path::new(stripped).components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(SecurityError::PathTraversalAttempt);
        }
        if matches!(component, std::path::Component::RootDir) {
            return Err(SecurityError::PathTraversalAttempt);
        }
    }
    let target = jail_root.join(stripped);
    let canonical = target.canonicalize().map_err(|_| SecurityError::InvalidPath)?;
    if !canonical.starts_with(jail_root) {
        return Err(SecurityError::OutsideJail);
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_root() -> PathBuf {
        let p = std::env::temp_dir().join("matchbox_test_security");
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p.canonicalize().unwrap()
    }

    #[test]
    fn valid_path_resolves() {
        let root = tmp_root();
        fs::create_dir(root.join("subdir")).unwrap();
        let result = resolve_sandboxed_path(&root, "subdir");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), root.join("subdir"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_parent_dir_traversal() {
        let root = tmp_root();
        let result = resolve_sandboxed_path(&root, "../etc");
        assert!(matches!(result, Err(SecurityError::PathTraversalAttempt)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_absolute_path_treated_as_rel() {
        let root = tmp_root();
        let result = resolve_sandboxed_path(&root, "/etc");
        // Absolute path is stripped to "etc" which doesn't exist in jail
        assert!(matches!(result, Err(SecurityError::InvalidPath)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_nonexistent() {
        let root = tmp_root();
        let result = resolve_sandboxed_path(&root, "nonexistent");
        assert!(matches!(result, Err(SecurityError::InvalidPath)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn strips_leading_slashes() {
        let root = tmp_root();
        fs::create_dir(root.join("sub")).unwrap();
        let result = resolve_sandboxed_path(&root, "//sub");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), root.join("sub"));
        let _ = fs::remove_dir_all(&root);
    }
}