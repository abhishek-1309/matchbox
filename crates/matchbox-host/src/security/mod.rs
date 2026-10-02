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
    let jail_root = jail_root.canonicalize().map_err(|_| SecurityError::InvalidPath)?;
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
    if !canonical.starts_with(&jail_root) {
        return Err(SecurityError::OutsideJail);
    }
    Ok(canonical)
}

/// Path for a file or directory that may not exist yet.
/// The parent must already exist inside the jail. The final component is the new name.
pub fn resolve_creatable_path(jail_root: &Path, guest_path_str: &str) -> Result<PathBuf, SecurityError> {
    if guest_path_str.contains('\0') {
        return Err(SecurityError::InvalidPath);
    }
    let jail_root = jail_root.canonicalize().map_err(|_| SecurityError::InvalidPath)?;
    let stripped = guest_path_str.trim_start_matches('/');
    if stripped.is_empty() {
        return Err(SecurityError::InvalidPath);
    }

    let mut parts = Vec::new();
    for component in Path::new(stripped).components() {
        match component {
            std::path::Component::Normal(name) => parts.push(name.to_os_string()),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir | std::path::Component::RootDir => {
                return Err(SecurityError::PathTraversalAttempt);
            }
            std::path::Component::Prefix(_) => return Err(SecurityError::InvalidPath),
        }
    }
    let Some((name, parents)) = parts.split_last() else {
        return Err(SecurityError::InvalidPath);
    };

    let mut parent = jail_root.clone();
    for dir in parents {
        parent = parent.join(dir).canonicalize().map_err(|_| SecurityError::InvalidPath)?;
        if !parent.starts_with(&jail_root) {
            return Err(SecurityError::OutsideJail);
        }
    }

    let dest = parent.join(name);
    if dest.exists() {
        let canonical = dest.canonicalize().map_err(|_| SecurityError::InvalidPath)?;
        if !canonical.starts_with(&jail_root) {
            return Err(SecurityError::OutsideJail);
        }
        return Ok(canonical);
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp_root() -> PathBuf {
        let p = std::env::temp_dir().join("matchbox_test_security");
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p.canonicalize().unwrap_or(p)
    }

    #[test]
    fn valid_path_resolves() {
        let root = tmp_root();
        fs::create_dir(root.join("subdir")).unwrap();
        let result = resolve_sandboxed_path(&root, "subdir");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("subdir"));
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
        assert!(result.unwrap().ends_with("sub"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn creatable_path_under_jail() {
        let root = tmp_root();
        let result = resolve_creatable_path(&root, "new-file");
        assert!(result.is_ok());
        assert!(result.unwrap().ends_with("new-file"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn creatable_path_rejects_traversal() {
        let root = tmp_root();
        let result = resolve_creatable_path(&root, "../escape");
        assert!(matches!(result, Err(SecurityError::PathTraversalAttempt)));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn creatable_path_requires_existing_parent() {
        let root = tmp_root();
        let result = resolve_creatable_path(&root, "missing/new-file");
        assert!(matches!(result, Err(SecurityError::InvalidPath)));
        let _ = fs::remove_dir_all(&root);
    }
}
