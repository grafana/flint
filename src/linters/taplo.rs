pub(crate) fn normalize_nonverbose_failure_output(
    argv: &[String],
    stdout: &[u8],
    stderr: &[u8],
) -> (Vec<u8>, Vec<u8>) {
    let raw = format!(
        "{}{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );
    let mut error_lines: Vec<String> = raw
        .lines()
        .filter(|line| line.starts_with("ERROR"))
        .map(ToOwned::to_owned)
        .collect();

    if error_lines.is_empty()
        && let Some(target) = argv.last()
    {
        error_lines.push(format!(
            "ERROR taplo:format_files: the file is not properly formatted path=\"{target}\""
        ));
    }

    if !error_lines.is_empty()
        && !error_lines.iter().any(|line| {
            line == "ERROR operation failed error=some files were not properly formatted"
        })
    {
        error_lines.push(
            "ERROR operation failed error=some files were not properly formatted".to_string(),
        );
    }

    let stderr = if error_lines.is_empty() {
        Vec::new()
    } else {
        format!("{}\n", error_lines.join("\n")).into_bytes()
    };

    (Vec::new(), stderr)
}
