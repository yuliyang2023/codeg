use super::git_workbench_store::{directory, read, write};
use crate::app_error::AppCommandError;
use serde::{Deserialize, Serialize};
use std::{sync::Mutex, time::Instant};
static LOG: Mutex<()> = Mutex::new(());
#[derive(Clone, Serialize, Deserialize)]
pub struct GitConsoleEntry {
    pub id: String,
    pub time: String,
    pub repository: String,
    pub command: Vec<String>,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}

fn redact(value: &str) -> String {
    static URL: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)(https?://)[^/@\s]+@").unwrap());
    static SECRET: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
        regex::Regex::new(r"(?im)((?:authorization|password|access_token|private_token|token)[=: ]+)[^\r\n\s]+(?: [^\r\n]+)?").unwrap()
    });
    SECRET
        .replace_all(&URL.replace_all(value, "${1}[redacted]@"), "${1}[redacted]")
        .into_owned()
}
fn bounded(bytes: &[u8]) -> String {
    redact(
        &String::from_utf8_lossy(bytes)
            .chars()
            .take(16_384)
            .collect::<String>(),
    )
}
pub(crate) fn record(
    command: &tokio::process::Command,
    started: Instant,
    result: &std::io::Result<std::process::Output>,
) {
    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let command = command.as_std();
    let args: Vec<_> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let mut cwd = command
        .get_current_dir()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    for pair in args.windows(2) {
        if pair[0] == "-C" {
            cwd = std::path::PathBuf::from(&pair[1]);
        }
    }
    if !cwd.is_dir() {
        return;
    }
    let Ok(_guard) = LOG.lock() else {
        return;
    };
    let Ok(dir) = directory(&cwd.to_string_lossy()) else {
        return;
    };
    let file = dir.join("git-console.json");
    let Ok(mut entries) = read::<Vec<GitConsoleEntry>>(&file) else {
        return;
    };
    let (code, stdout, stderr, truncated) = match result {
        Ok(out) => (
            out.status.code(),
            bounded(&out.stdout),
            bounded(&out.stderr),
            out.stdout.len() > 16_384 || out.stderr.len() > 16_384,
        ),
        Err(err) => (None, String::new(), redact(&err.to_string()), false),
    };
    entries.push(GitConsoleEntry {
        id: uuid::Uuid::new_v4().to_string(),
        time: chrono::Utc::now().to_rfc3339(),
        repository: cwd.to_string_lossy().into_owned(),
        command: std::iter::once("git".to_string())
            .chain(args.into_iter().map(|arg| redact(&arg)))
            .collect(),
        duration_ms,
        exit_code: code,
        stdout,
        stderr,
        truncated,
    });
    if entries.len() > 200 {
        entries.drain(..entries.len() - 200);
    }
    if let Err(err) = write(&file, &entries) {
        tracing::debug!("Git Console write failed: {}", err);
    }
}
pub(crate) async fn output(
    command: &mut tokio::process::Command,
) -> std::io::Result<std::process::Output> {
    let started = Instant::now();
    let result = command.output().await;
    record(command, started, &result);
    result
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_console_list(path: String) -> Result<Vec<GitConsoleEntry>, AppCommandError> {
    let _guard = LOG
        .lock()
        .map_err(|_| AppCommandError::invalid_input("Git Console is busy"))?;
    let mut entries: Vec<GitConsoleEntry> = read(&directory(&path)?.join("git-console.json"))?;
    entries.reverse();
    Ok(entries)
}
#[cfg_attr(feature = "tauri-runtime", tauri::command)]
pub async fn git_console_clear(path: String) -> Result<(), AppCommandError> {
    let _guard = LOG
        .lock()
        .map_err(|_| AppCommandError::invalid_input("Git Console is busy"))?;
    write(
        &directory(&path)?.join("git-console.json"),
        &Vec::<GitConsoleEntry>::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn records_success_failure_and_redacts_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap().to_string();
        let mut command = crate::process::tokio_command("git");
        command.args(["status"]).current_dir(&path);
        let out = output(&mut command).await.unwrap();
        assert!(!out.status.success());
        let entries = git_console_list(path.clone()).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_ne!(entries[0].exit_code, Some(0));
        assert!(entries[0].stderr.contains("not a git repository"));
        assert!(!redact("https://secret@example.test/repo").contains("secret"));
        assert!(!redact("Authorization: Bearer secret\ntoken=secret").contains("secret"));
        git_console_clear(path.clone()).await.unwrap();
        assert!(git_console_list(path).await.unwrap().is_empty());
    }
}
