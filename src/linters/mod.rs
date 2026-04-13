pub mod license_header;
pub mod lychee;
pub mod renovate_deps;

/// Build a [`tokio::process::Command`] for the given argv.
///
/// On Windows, mise shims come in several forms depending on whether
/// `mise-shim.exe` is available:
///
/// - **exe mode**: `<tool>.exe` is a copy of `mise-shim.exe` — detected as PE
///   (MZ magic) and spawned directly via `CreateProcessW`.
/// - **file mode** (fallback when `mise-shim.exe` is absent): `<tool>` is an
///   extensionless bash script (`#!/bin/bash`) that calls `mise exec`. cmd.exe
///   cannot execute these, so we invoke them via `bash`.
/// - **`.cmd` shims** (older mise behaviour): routed through `cmd.exe /C`.
///
/// Some tools are self-executing JARs (e.g. ktlint) that cmd.exe cannot run
/// at all. When `windows_java_jar` is true, the binary is resolved to its
/// full path and invoked as `java -jar <path>`.
pub fn spawn_command(argv: &[String], windows_java_jar: bool) -> tokio::process::Command {
    #[cfg(windows)]
    {
        if windows_java_jar {
            if let Some(path) = find_file_in_path(&argv[0]) {
                let mut cmd = tokio::process::Command::new("java");
                cmd.arg("-jar").arg(path).args(&argv[1..]);
                return cmd;
            }
        } else if let Some(path) = find_pe_binary(&argv[0]) {
            // Native PE binary (exe-mode shim or unextensioned binary).
            let mut cmd = tokio::process::Command::new(path);
            cmd.args(&argv[1..]);
            return cmd;
        } else if let Some(path) = find_bash_shim(&argv[0]) {
            // File-mode mise shim: an extensionless bash script.
            // Git Bash (bash.exe) is available on all Windows CI runners.
            let mut cmd = tokio::process::Command::new("bash");
            cmd.arg(path).args(&argv[1..]);
            return cmd;
        }
        // Fall back to cmd.exe for .cmd shims (older mise behaviour).
        let mut cmd = tokio::process::Command::new("cmd.exe");
        cmd.arg("/C").args(argv);
        cmd
    }
    #[cfg(not(windows))]
    {
        let _ = windows_java_jar;
        let mut cmd = tokio::process::Command::new(&argv[0]);
        cmd.args(&argv[1..]);
        cmd
    }
}

/// On Windows, look for `binary` (exact name, no extension) in each PATH
/// directory. If found and it starts with the PE magic bytes `MZ`, return
/// its full path so it can be executed directly via `CreateProcessW`.
#[cfg(windows)]
fn find_pe_binary(binary: &str) -> Option<std::path::PathBuf> {
    use std::io::Read;
    let path_var = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if !candidate.is_file() {
            continue;
        }
        let is_pe = std::fs::File::open(&candidate)
            .and_then(|mut f| {
                let mut buf = [0u8; 2];
                f.read_exact(&mut buf)?;
                Ok(buf == [b'M', b'Z'])
            })
            .unwrap_or(false);
        if is_pe {
            return Some(candidate);
        }
    }
    None
}

/// On Windows, look for `binary` in PATH and return its full path if it looks
/// like a bash script (starts with `#!`). Used to detect mise "file" mode
/// shims, which are extensionless `#!/bin/bash` scripts that must be invoked
/// via `bash` rather than `cmd.exe`.
#[cfg(windows)]
fn find_bash_shim(binary: &str) -> Option<std::path::PathBuf> {
    use std::io::Read;
    let path_var = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if !candidate.is_file() {
            continue;
        }
        let is_bash = std::fs::File::open(&candidate)
            .and_then(|mut f| {
                let mut buf = [0u8; 2];
                f.read_exact(&mut buf).map(|_| buf == [b'#', b'!'])
            })
            .unwrap_or(false);
        if is_bash {
            return Some(candidate);
        }
    }
    None
}

/// On Windows, return the full path of `binary` from PATH without inspecting
/// its contents. Used for self-executing JARs where the caller already knows
/// the invocation style (i.e. `windows_java_jar` is set in the registry).
#[cfg(windows)]
fn find_file_in_path(binary: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    std::env::split_paths(&path_var).find_map(|dir| {
        let candidate = dir.join(binary);
        candidate.is_file().then_some(candidate)
    })
}

/// Output from a single linter run.
pub struct LinterOutput {
    pub ok: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl LinterOutput {
    pub fn err(stderr: impl Into<Vec<u8>>) -> Self {
        Self {
            ok: false,
            stdout: vec![],
            stderr: stderr.into(),
        }
    }
}
