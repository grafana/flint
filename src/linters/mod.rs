pub mod license_header;
pub mod lychee;
pub mod renovate_deps;

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
