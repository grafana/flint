pub mod license_header;
pub mod lychee;
pub mod renovate_deps;

/// Build a [`tokio::process::Command`] for the given argv.
///
/// On Windows, mise shims are `.cmd` files that cannot be spawned directly
/// via `CreateProcessW`. However, some tools (e.g. ktlint) are native PE
/// binaries without a `.exe` extension that also cannot run via cmd.exe
/// (the shim fails). We check for a PE header (MZ magic) to distinguish:
/// - PE binary without extension → execute directly by full path
/// - Everything else → route through `cmd.exe /C` to handle `.cmd` shims
pub fn spawn_command(argv: &[String]) -> tokio::process::Command {
    #[cfg(windows)]
    {
        if let Some(full_path) = find_pe_binary(&argv[0]) {
            let mut cmd = tokio::process::Command::new(full_path);
            cmd.args(&argv[1..]);
            return cmd;
        }
        let mut cmd = tokio::process::Command::new("cmd.exe");
        cmd.arg("/C").args(argv);
        cmd
    }
    #[cfg(not(windows))]
    {
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
        // Check for Windows PE magic bytes (MZ header)
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
