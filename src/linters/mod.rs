pub mod biome;
pub mod env;
pub mod flint_setup;
pub mod kube_linter;
pub mod license_header;
pub mod lychee;
pub mod renovate_deps;
pub mod rumdl;
pub mod rustfmt;
pub mod taplo;
pub mod typos;
pub mod yamllint;

pub use crate::registry::LinterOutput;

/// Build a [`tokio::process::Command`] for the given argv.
///
/// On Windows, mise shims are `.cmd` files that cannot be spawned directly
/// via `CreateProcessW`. However, some tools (e.g. ktlint) are native PE
/// binaries without a `.exe` extension that also cannot run via cmd.exe
/// (the shim fails). We check for a PE header (MZ magic) to distinguish:
/// - PE binary without extension → execute directly by full path
/// - Everything else → route through `cmd.exe /C` to handle `.cmd` shims
///
/// Self-executing JARs (e.g. ktlint and Checkstyle) need to be invoked through
/// the JVM. When `java_jar` is true the resolved tool path is invoked as
/// `java -jar <path>` on every platform.
pub fn spawn_command(argv: &[String], java_jar: bool) -> tokio::process::Command {
    #[cfg(windows)]
    {
        if java_jar {
            if let Some(path) = find_file_in_path(&argv[0]) {
                let mut cmd = tokio::process::Command::new("java");
                cmd.arg("-jar").arg(path).args(&argv[1..]);
                return cmd;
            }
        } else if let Some(path) = find_pe_binary(&argv[0]) {
            let mut cmd = tokio::process::Command::new(path);
            cmd.args(&argv[1..]);
            return cmd;
        }
        let mut cmd = tokio::process::Command::new("cmd.exe");
        cmd.arg("/C").args(argv);
        cmd
    }
    #[cfg(not(windows))]
    {
        if java_jar {
            let jar = find_file_in_path(&argv[0]).unwrap_or_else(|| argv[0].clone().into());
            let mut cmd = tokio::process::Command::new("java");
            cmd.arg("-jar").arg(jar).args(&argv[1..]);
            return cmd;
        }
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

/// On Windows, return the full path of `binary` from PATH without inspecting
/// its contents. Used for self-executing JARs where the caller already knows
/// the invocation style (i.e. `java_jar` is set in the registry).
#[cfg(windows)]
fn find_file_in_path(binary: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    std::env::split_paths(&path_var).find_map(|dir| {
        let candidate = dir.join(binary);
        candidate.is_file().then_some(candidate)
    })
}

#[cfg(not(windows))]
fn find_file_in_path(binary: &str) -> Option<std::path::PathBuf> {
    let path_var = std::env::var("PATH").ok()?;
    std::env::split_paths(&path_var).find_map(|dir| {
        let candidate = dir.join(binary);
        candidate.is_file().then_some(candidate)
    })
}
