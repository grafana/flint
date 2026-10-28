pub(crate) const CI_ENV_VARS: &[&str] =
    &["CI", "GITHUB_ACTIONS", "GITHUB_ACTION", "GITHUB_WORKFLOW"];
pub(crate) const GITHUB_COM_TOKEN_ENV: &str = "GITHUB_COM_TOKEN";
pub(crate) const GITHUB_TOKEN_ENV: &str = "GITHUB_TOKEN";
const GITHUB_SERVER_URL_ENV: &str = "GITHUB_SERVER_URL";

/// Returns an explicitly configured token, falling back to the user's GitHub CLI
/// credentials without ever exposing command output in diagnostics.
pub(crate) fn github_token() -> Option<String> {
    github_token_from(|name| std::env::var(name).ok(), gh_auth_token)
}

pub(crate) fn github_token_from<F, G>(env: F, gh_token: G) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
    G: FnOnce() -> Option<String>,
{
    env(GITHUB_TOKEN_ENV)
        .filter(|token| !token.trim().is_empty())
        .or_else(gh_token)
}

pub(crate) fn gh_auth_token() -> Option<String> {
    let hostname = github_server_hostname_from(|name| std::env::var(name).ok())?;
    static GH_TOKENS: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, Option<String>>>,
    > = std::sync::OnceLock::new();
    let cache = GH_TOKENS.get_or_init(Default::default);
    let mut cache = cache.lock().ok()?;
    if let Some(token) = cache.get(&hostname) {
        return token.clone();
    }
    let token = gh_auth_token_with(&hostname, |command, args| {
        std::process::Command::new(command).args(args).output()
    });
    cache.insert(hostname, token.clone());
    token
}

/// Resolve the GitHub server host corresponding to GITHUB_SERVER_URL.
pub(crate) fn github_server_hostname_from<F>(env: F) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(server_url) = env(GITHUB_SERVER_URL_ENV) else {
        return Some("github.com".into());
    };
    let server_url = server_url.trim();
    let (scheme, authority) = server_url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return None;
    }
    let authority = authority.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains('@') || authority.chars().any(char::is_whitespace)
    {
        return None;
    }
    let (host, port) = if authority.starts_with('[') {
        let (literal, suffix) = authority.split_once(']')?;
        let address = literal.strip_prefix('[')?;
        address.parse::<std::net::Ipv6Addr>().ok()?;
        let port = if suffix.is_empty() {
            String::new()
        } else {
            format!(":{}", suffix.strip_prefix(':')?.parse::<u16>().ok()?)
        };
        (format!("[{address}]"), port)
    } else {
        let (name, port) = match authority.split_once(':') {
            Some((name, suffix)) => (name, format!(":{}", suffix.parse::<u16>().ok()?)),
            None => (authority, String::new()),
        };
        if name.is_empty()
            || name.split('.').any(|label| {
                label.is_empty()
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
            })
        {
            return None;
        }
        (name.to_ascii_lowercase(), port)
    };
    if host.is_empty() {
        return None;
    }
    Some(format!("{host}{port}"))
}

fn gh_auth_token_with<F, E>(hostname: &str, run: F) -> Option<String>
where
    F: FnOnce(&str, &[&str]) -> Result<std::process::Output, E>,
{
    let output = run("gh", &["auth", "token", "--hostname", hostname]).ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

pub(crate) fn is_ci_from<F>(env: F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    CI_ENV_VARS.iter().any(|name| env_truthy(&env, name))
}

pub(crate) fn env_non_empty<F>(env: &F, name: &str) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    env(name)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

pub(crate) fn github_token_available_with<F, G>(env: &F, gh_available: G) -> bool
where
    F: Fn(&str) -> Option<String>,
    G: FnOnce() -> bool,
{
    env_non_empty(env, GITHUB_TOKEN_ENV) || gh_available()
}

pub(crate) fn renovate_github_token_available_with<F, G>(env: &F, gh_available: G) -> bool
where
    F: Fn(&str) -> Option<String>,
    G: FnOnce() -> bool,
{
    env_non_empty(env, GITHUB_COM_TOKEN_ENV)
        || env_non_empty(env, GITHUB_TOKEN_ENV)
        || gh_available()
}

pub(crate) fn token_warning(check_name: &str, token_names: &str) -> String {
    format!(
        "flint: warning: {token_names} is not set; {check_name} GitHub requests may be rate limited"
    )
}

pub(crate) fn env_truthy<F>(env: &F, name: &str) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    env(name)
        .map(|value| {
            let value = value.trim();
            !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn detects_truthy_ci_env() {
        let vars = HashMap::from([("CI".to_string(), "true".to_string())]);

        assert!(is_ci_from(|name| vars.get(name).cloned()));
    }

    #[test]
    fn ignores_false_ci_env() {
        let vars = HashMap::from([("CI".to_string(), "false".to_string())]);

        assert!(!is_ci_from(|name| vars.get(name).cloned()));
    }

    #[test]
    fn detects_non_empty_github_token() {
        let vars = HashMap::from([("GITHUB_TOKEN".to_string(), "token".to_string())]);

        assert!(github_token_available_with(
            &|name| vars.get(name).cloned(),
            || false
        ));
    }

    #[test]
    fn detects_renovate_github_com_token() {
        let vars = HashMap::from([("GITHUB_COM_TOKEN".to_string(), "token".to_string())]);

        assert!(renovate_github_token_available_with(
            &|name| vars.get(name).cloned(),
            || false
        ));
    }

    #[test]
    fn gh_token_resolution_handles_success_failure_and_empty_output() {
        let output = |success: bool, stdout: &[u8]| std::process::Output {
            status: test_exit_status(success),
            stdout: stdout.to_vec(),
            stderr: b"must not be logged".to_vec(),
        };
        let run = |success, stdout| {
            move |command: &str, args: &[&str]| {
                assert_eq!(command, "gh");
                assert_eq!(args, ["auth", "token", "--hostname", "github.com"]);
                Ok::<_, std::io::Error>(output(success, stdout))
            }
        };
        assert_eq!(
            gh_auth_token_with("github.com", run(true, b"secret\n")),
            Some("secret".into())
        );
        assert_eq!(
            gh_auth_token_with("github.com", run(false, b"secret\n")),
            None
        );
        assert_eq!(gh_auth_token_with("github.com", run(true, b"  \n")), None);
        assert_eq!(
            gh_auth_token_with("github.com", |_, _| Err::<std::process::Output, _>(
                std::io::ErrorKind::NotFound
            )),
            None
        );
        let output = std::process::Output {
            status: test_exit_status(true),
            stdout: b"secret\n".to_vec(),
            stderr: Vec::new(),
        };
        assert_eq!(
            gh_auth_token_with("ghe.example.test", |command, args| {
                assert_eq!(command, "gh");
                assert_eq!(args, ["auth", "token", "--hostname", "ghe.example.test"]);
                Ok::<_, std::io::Error>(output)
            }),
            Some("secret".into())
        );
    }

    #[test]
    fn resolves_gh_hostname_from_server_url() {
        assert_eq!(
            github_server_hostname_from(|_| None).as_deref(),
            Some("github.com")
        );
        assert_eq!(
            github_server_hostname_from(|_| Some("https://github.com/".into())).as_deref(),
            Some("github.com")
        );
        assert_eq!(
            github_server_hostname_from(|_| Some("https://ghe.example.test/".into())).as_deref(),
            Some("ghe.example.test")
        );
        assert_eq!(
            github_server_hostname_from(|_| Some("https://GHE.Example.test:8443/".into()))
                .as_deref(),
            Some("ghe.example.test:8443")
        );
        assert_eq!(
            github_server_hostname_from(|_| Some("https://[2001:db8::1]:8443/".into())).as_deref(),
            Some("[2001:db8::1]:8443")
        );
        assert_eq!(github_server_hostname_from(|_| Some("wat".into())), None);
        assert_eq!(
            github_server_hostname_from(|_| Some("file:///tmp".into())),
            None
        );
        assert_eq!(
            github_server_hostname_from(|_| Some("https://ghe.example.test:bad/".into())),
            None
        );
        assert_eq!(
            github_server_hostname_from(|_| Some("https://ghe.example.test:65536/".into())),
            None
        );
        assert_eq!(
            github_server_hostname_from(|_| Some("https://[not-ipv6]/".into())),
            None
        );
    }

    #[test]
    fn gh_server_hostname_ignores_conflicting_api_url() {
        let vars = HashMap::from([
            (
                GITHUB_SERVER_URL_ENV.to_string(),
                "https://ghe.example.test".to_string(),
            ),
            (
                "GITHUB_API_URL".to_string(),
                "https://api.other.test".to_string(),
            ),
        ]);
        assert_eq!(
            github_server_hostname_from(|name| vars.get(name).cloned()).as_deref(),
            Some("ghe.example.test")
        );
    }

    #[cfg(unix)]
    fn test_exit_status(success: bool) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(if success { 0 } else { 1 << 8 })
    }

    #[cfg(windows)]
    fn test_exit_status(success: bool) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(if success { 0 } else { 1 })
    }

    #[test]
    fn explicit_github_token_precedes_gh_fallback() {
        let vars = HashMap::from([(GITHUB_TOKEN_ENV.to_string(), "explicit".to_string())]);
        assert_eq!(
            github_token_from(
                |name| vars.get(name).cloned(),
                || panic!("gh fallback must not run when an explicit token exists"),
            ),
            Some("explicit".into())
        );
        assert_eq!(
            github_token_from(|_| None, || Some("gh".into())),
            Some("gh".into())
        );
    }
}
