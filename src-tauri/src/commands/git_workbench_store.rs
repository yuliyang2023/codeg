use crate::app_error::AppCommandError;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn directory(root: &str) -> Result<PathBuf, AppCommandError> {
    let canonical = std::fs::canonicalize(root).map_err(AppCommandError::io)?;
    if !canonical.is_dir() {
        return Err(AppCommandError::invalid_input(
            "Select a workspace directory",
        ));
    }
    #[cfg(not(test))]
    let base = crate::paths::codeg_logs_root()
        .parent()
        .unwrap_or(Path::new("."))
        .join("workbench");
    #[cfg(test)]
    let base = std::env::temp_dir().join("codeg-workbench-tests");
    let directory = base.join(digest(canonical.to_string_lossy().as_bytes()));
    std::fs::create_dir_all(&directory).map_err(AppCommandError::io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .map_err(AppCommandError::io)?;
    }
    Ok(directory)
}

pub(crate) fn read<T: DeserializeOwned + Default>(file: &Path) -> Result<T, AppCommandError> {
    match std::fs::read(file) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|err| {
            AppCommandError::invalid_input(format!("Invalid workbench metadata: {err}"))
        }),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
        Err(err) => Err(AppCommandError::io(err)),
    }
}

pub(crate) fn write<T: Serialize>(file: &Path, value: &T) -> Result<(), AppCommandError> {
    let bytes =
        serde_json::to_vec(value).map_err(|err| AppCommandError::invalid_input(err.to_string()))?;
    super::folders::atomic_write_text(file, &bytes)
}

pub(crate) fn is_metadata_path(path: &Path) -> bool {
    #[cfg(not(test))]
    let base = crate::paths::codeg_logs_root()
        .parent()
        .unwrap_or(Path::new("."))
        .join("workbench");
    #[cfg(test)]
    let base = std::env::temp_dir().join("codeg-workbench-tests");
    path.starts_with(base)
}
