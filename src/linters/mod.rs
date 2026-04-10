pub mod license_header;
pub mod lychee;
pub mod renovate_deps;

/// Build a [`tokio::process::Command`] for the given argv.
///
/// On Windows, mise shims are `.cmd` files that cannot be spawned directly
/// via `CreateProcessW`. Route everything through `cmd.exe /C` so both
/// `.cmd` shims and native `.exe` binaries are handled uniformly.
pub fn spawn_command(argv: &[String]) -> tokio::process::Command {
    #[cfg(windows)]
    {
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
