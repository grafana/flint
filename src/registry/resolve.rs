/// Returns true if `bin_name` exists as a file in any directory in `path_var`
/// (a `:`-separated PATH string). Accepts the PATH string as a parameter so
/// callers can substitute a test-controlled path without mutating env vars.
#[cfg(not(windows))]
pub fn binary_on_path_var(bin_name: &str, path_var: &str) -> bool {
    std::env::split_paths(path_var).any(|dir| dir.join(bin_name).is_file())
}

/// Returns true if `bin_name` is found in the current `PATH`.
/// On Windows this shells out to `where` so PATHEXT entries such as `.exe`
/// and `.cmd` are resolved the same way the shell does.
pub fn binary_on_path(bin_name: &str) -> bool {
    #[cfg(windows)]
    {
        return std::process::Command::new("where")
            .arg(bin_name)
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
    }

    #[cfg(not(windows))]
    binary_on_path_var(bin_name, &std::env::var("PATH").unwrap_or_default())
}
