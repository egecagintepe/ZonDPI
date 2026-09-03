//! Environment-aware runtime path resolution for binaries, profiles, logs, and configuration.

use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use zondpi_packet_engine::ProfileDefinition;

#[derive(Error, Debug)]
pub enum PathError {
    #[error(
        "Profile identifier '{0}' contains invalid characters or directory traversal attempts"
    )]
    InvalidProfileId(String),
    #[error("Profile '{0}' not found in candidate paths: {1:?}")]
    ProfileNotFound(String, Vec<PathBuf>),
    #[error("Failed to determine current executable path: {0}")]
    ExecutablePath(#[from] std::io::Error),
}

/// Discovers and manages filesystem layout for ZonDPI service in both production and development environments.
#[derive(Debug, Clone)]
pub struct RuntimePaths {
    pub install_root: PathBuf,
    pub data_root: PathBuf,
    pub log_root: PathBuf,
    pub profile_roots: Vec<PathBuf>,
}

impl RuntimePaths {
    /// Detects runtime paths automatically based on current executable and environment.
    pub fn discover() -> Result<Self, PathError> {
        let exe_path = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let install_root = exe_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        // Check ProgramData on Windows
        let program_data = std::env::var("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("C:\\ProgramData"));
        let data_root = program_data.join("ZonDPI");
        let log_root = data_root.join("logs");

        let mut profile_roots = Vec::new();
        // 1. Production ProgramData profiles
        profile_roots.push(data_root.join("profiles"));
        // 2. Install dir profiles
        profile_roots.push(install_root.join("profiles").join("turkey"));
        profile_roots.push(install_root.join("profiles"));

        // 3. Search up from current directory and executable directory for workspace profiles
        if let Ok(cwd) = std::env::current_dir() {
            let mut curr = Some(cwd.as_path());
            while let Some(dir) = curr {
                let p_turkey = dir.join("profiles").join("turkey");
                if p_turkey.is_dir() && !profile_roots.contains(&p_turkey) {
                    profile_roots.push(p_turkey);
                }
                let p_dir = dir.join("profiles");
                if p_dir.is_dir() && !profile_roots.contains(&p_dir) {
                    profile_roots.push(p_dir);
                }
                curr = dir.parent();
            }
        }

        let mut curr_exe = Some(exe_path.as_path());
        while let Some(dir) = curr_exe {
            let p_turkey = dir.join("profiles").join("turkey");
            if p_turkey.is_dir() && !profile_roots.contains(&p_turkey) {
                profile_roots.push(p_turkey);
            }
            let p_dir = dir.join("profiles");
            if p_dir.is_dir() && !profile_roots.contains(&p_dir) {
                profile_roots.push(p_dir);
            }
            curr_exe = dir.parent();
        }

        Ok(Self {
            install_root,
            data_root,
            log_root,
            profile_roots,
        })
    }

    /// Creates runtime paths explicitly for testing or custom deployments.
    pub fn for_test(base_dir: &Path) -> Self {
        let install_root = base_dir.to_path_buf();
        let data_root = base_dir.join("data");
        let log_root = data_root.join("logs");
        let profile_roots = vec![
            base_dir.join("profiles").join("turkey"),
            base_dir.join("profiles"),
        ];

        Self {
            install_root,
            data_root,
            log_root,
            profile_roots,
        }
    }

    /// Resolves a profile ID to an existing `.json` file safely without directory traversal.
    pub fn resolve_profile_path(&self, profile_id: &str) -> Result<PathBuf, PathError> {
        // Prevent path traversal
        if profile_id.is_empty()
            || profile_id.contains('/')
            || profile_id.contains('\\')
            || profile_id.contains("..")
            || !profile_id
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(PathError::InvalidProfileId(profile_id.to_string()));
        }

        let file_name = if profile_id.ends_with(".json") {
            profile_id.to_string()
        } else {
            format!("{}.json", profile_id)
        };

        let mut searched = Vec::new();

        // 1. Direct filename match
        for root in &self.profile_roots {
            let candidate = root.join(&file_name);
            searched.push(candidate.clone());
            if candidate.is_file() {
                return Ok(candidate);
            }
        }

        // 2. Scan for JSON file whose `id` field matches profile_id
        for root in &self.profile_roots {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(def) = serde_json::from_str::<ProfileDefinition>(&content) {
                                if def.id == profile_id {
                                    return Ok(path);
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(PathError::ProfileNotFound(profile_id.to_string(), searched))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rejects_directory_traversal() {
        let paths = RuntimePaths::discover().expect("discover paths");
        assert!(paths.resolve_profile_path("../secrets").is_err());
        assert!(paths.resolve_profile_path("..\\secrets").is_err());
        assert!(paths.resolve_profile_path("sub/profile").is_err());
        assert!(paths.resolve_profile_path("").is_err());
    }

    #[test]
    fn test_resolves_valid_turktelekom_profile() {
        let paths = RuntimePaths::discover().expect("discover paths");
        let res = paths.resolve_profile_path("turktelekom");
        assert!(res.is_ok(), "turktelekom profile should resolve: {:?}", res);
        let path = res.unwrap();
        assert!(path.is_file());

        // Also test by profile ID
        let res_id = paths.resolve_profile_path("turkey-default");
        assert!(
            res_id.is_ok(),
            "turkey-default profile id should resolve: {:?}",
            res_id
        );
    }
}
