// Inspired by the Tauri project implementation
use std::path::PathBuf;
use std::process::Command as StdCommand;

use flow_like_types::tokio::process::{self, Command};

pub fn executable_path() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?;
    let parent = path.parent()?;
    Some(parent.to_path_buf())
}

fn side_car_path(command: &PathBuf) -> flow_like_types::Result<PathBuf> {
    let executable =
        executable_path().ok_or(flow_like_types::anyhow!("Could not get executable path"))?;
    #[cfg(windows)]
    return Ok(executable.join(&command).with_extension("exe"));
    #[cfg(not(windows))]
    return Ok(executable.join(command));
}

/// Creates a sidecar command to run a script or executable.
/// If `with_bash` is true, it will run the command using `bash`. Important for some Systems and binaries
/// Otherwise, it will run the command directly.
/// Returns a `flow_like_types::Result<StdCommand>`
/// which can be used to execute the command asynchronously.
pub async fn sidecar(
    command: &PathBuf,
    with_bash: Option<bool>,
) -> flow_like_types::Result<StdCommand> {
    let path = side_car_path(command)?;

    if !path.exists() {
        return Err(flow_like_types::anyhow!(
            "Sidecar not found at path: {:?}",
            path
        ));
    }

    if !path.is_file() {
        return Err(flow_like_types::anyhow!(
            "Sidecar is not a file: {:?}",
            path
        ));
    }

    let with_bash = with_bash.unwrap_or(false);

    if with_bash {
        #[cfg(target_os = "linux")]
        {
            let mut sidecar = StdCommand::new("bash");
            sidecar.arg(path);
            return Ok(sidecar);
        }
    }

    let sidecar = StdCommand::new(path);
    Ok(sidecar)
}

//
pub async fn async_sidecar(command: &PathBuf) -> flow_like_types::Result<Command> {
    let path = side_car_path(command)?;

    if !path.exists() {
        return Err(flow_like_types::anyhow!(
            "Sidecar not found at path: {:?}",
            path
        ));
    }

    if !path.is_file() {
        return Err(flow_like_types::anyhow!(
            "Sidecar is not a file: {:?}",
            path
        ));
    }

    #[cfg(not(target_os = "linux"))]
    {
        let sidecar = process::Command::new(path);
        Ok(sidecar)
    }

    #[cfg(target_os = "linux")]
    {
        let mut sidecar = process::Command::new("bash");
        sidecar.arg(path);
        Ok(sidecar)
    }
}

// ==================== IDEAS ====================
// - Sidecar BIT function
