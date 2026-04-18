use std::collections::HashMap;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use crate::config::ShellConfig;
use crate::provider::ProviderError;

/// The output produced by a shell command execution.
#[derive(Debug, Clone)]
pub struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Resolves a shell name to its [`ShellConfig`] and executes a command string
/// through it with an optional timeout.
pub struct ShellRunner;

impl ShellRunner {
    /// Executes `command` using the shell identified by `shell_name`.
    ///
    /// The shell binary and its prefix args are taken from `shells`. The
    /// command string is appended as the final argument. A hard timeout of
    /// `timeout_secs` seconds is applied; exceeding it returns
    /// [`ProviderError::Timeout`].
    pub async fn run(
        shell_name: &str,
        command: &str,
        shells: &HashMap<String, ShellConfig>,
        timeout_secs: u64,
    ) -> Result<ShellOutput, ProviderError> {
        let shell = shells.get(shell_name).ok_or_else(|| {
            ProviderError::ExecutionFailed(format!("unknown shell: {shell_name}"))
        })?;

        let mut cmd = Command::new(&shell.command);
        for arg in &shell.args {
            cmd.arg(arg);
        }
        cmd.arg(command);

        // Hide console window on Windows (prevents cmd/powershell/wsl flash)
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        crate::logging::debug(&format!("[shell] {shell_name}: {} {} {command}",
            shell.command, shell.args.join(" "),
        ));

        let result = timeout(Duration::from_secs(timeout_secs), async {
            cmd.output()
                .await
                .map_err(|e| ProviderError::ExecutionFailed(e.to_string()))
        })
        .await;

        match result {
            Err(_elapsed) => Err(ProviderError::Timeout),
            Ok(Err(e)) => Err(e),
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                let exit_code = output.status.code().unwrap_or(-1);
                crate::logging::debug(&format!("[shell] exit={exit_code} stdout={} stderr={}",
                    &stdout[..stdout.len().min(200)],
                    &stderr[..stderr.len().min(200)],
                ));
                Ok(ShellOutput { stdout, stderr, exit_code })
            }
        }
    }

    /// Builds the argv vector that would be passed to the OS for a given shell
    /// and command string. Useful for unit tests without spawning a process.
    pub fn build_argv(shell: &ShellConfig, command: &str) -> Vec<String> {
        let mut argv = vec![shell.command.clone()];
        argv.extend(shell.args.clone());
        argv.push(command.to_string());
        argv
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_shells() -> HashMap<String, ShellConfig> {
        let mut map = HashMap::new();
        map.insert(
            "bash".to_string(),
            ShellConfig {
                command: "bash".to_string(),
                args: vec!["-c".to_string()],
            },
        );
        map.insert(
            "wsl".to_string(),
            ShellConfig {
                command: "wsl.exe".to_string(),
                args: vec!["-e".to_string(), "bash".to_string(), "-ic".to_string()],
            },
        );
        map
    }

    #[test]
    fn test_build_argv_no_prefix_args() {
        let shell = ShellConfig {
            command: "sh".to_string(),
            args: vec![],
        };
        let argv = ShellRunner::build_argv(&shell, "echo hello");
        assert_eq!(argv, vec!["sh", "echo hello"]);
    }

    #[test]
    fn test_build_argv_with_prefix_args() {
        let shells = make_shells();
        let shell = shells.get("bash").unwrap();
        let argv = ShellRunner::build_argv(shell, "echo hello");
        assert_eq!(argv, vec!["bash", "-c", "echo hello"]);
    }

    #[test]
    fn test_build_argv_multi_prefix_args() {
        let shells = make_shells();
        let shell = shells.get("wsl").unwrap();
        let argv = ShellRunner::build_argv(shell, "uptime");
        assert_eq!(argv, vec!["wsl.exe", "-e", "bash", "-ic", "uptime"]);
    }

    #[tokio::test]
    async fn test_run_returns_error_on_unknown_shell() {
        let shells = make_shells();
        let result = ShellRunner::run("nonexistent", "echo hi", &shells, 5).await;
        assert!(matches!(result, Err(ProviderError::ExecutionFailed(_))));
        if let Err(ProviderError::ExecutionFailed(msg)) = result {
            assert!(msg.contains("unknown shell: nonexistent"));
        }
    }
}
