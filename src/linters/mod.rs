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
        match find_executable_in_path(&argv[0]) {
            Some(WinBinary::Pe(path)) => {
                let mut cmd = tokio::process::Command::new(path);
                cmd.args(&argv[1..]);
                return cmd;
            }
            Some(WinBinary::Jar(path)) => {
                let mut cmd = tokio::process::Command::new("java");
                cmd.arg("-jar").arg(path).args(&argv[1..]);
                return cmd;
            }
            None => {}
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

/// What kind of executable was found in PATH on Windows.
#[cfg(windows)]
enum WinBinary {
    /// Native PE binary (MZ magic) — execute directly.
    Pe(std::path::PathBuf),
    /// Self-executing JAR (starts with `#!` and is large) — run via `java -jar`.
    Jar(std::path::PathBuf),
}

/// On Windows, look for `binary` (exact name, no extension) in each PATH
/// directory and classify it:
/// - MZ magic → native PE, run directly
/// - `#!` magic + large file (>1 MB) → self-executing JAR (e.g. ktlint), run via `java -jar`
#[cfg(windows)]
fn find_executable_in_path(binary: &str) -> Option<WinBinary> {
    use std::io::Read;
    let path_var = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if !candidate.is_file() {
            continue;
        }
        let mut buf = [0u8; 2];
        let read = std::fs::File::open(&candidate)
            .and_then(|mut f| f.read(&mut buf).map(|n| n))
            .unwrap_or(0);
        if read < 2 {
            continue;
        }
        if buf == [b'M', b'Z'] {
            return Some(WinBinary::Pe(candidate));
        }
        if buf == [b'#', b'!'] {
            // Self-executing JAR: shell script header prepended to a JAR.
            // A real script would be tiny; a self-executing JAR is many MB.
            if std::fs::metadata(&candidate)
                .map(|m| m.len() > 1_000_000)
                .unwrap_or(false)
            {
                return Some(WinBinary::Jar(candidate));
            }
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
