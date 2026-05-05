pub(crate) const CI_ENV_VARS: &[&str] =
    &["CI", "GITHUB_ACTIONS", "GITHUB_ACTION", "GITHUB_WORKFLOW"];
pub(crate) const GITHUB_COM_TOKEN_ENV: &str = "GITHUB_COM_TOKEN";
pub(crate) const GITHUB_TOKEN_ENV: &str = "GITHUB_TOKEN";

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

pub(crate) fn github_token_available<F>(env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    env_non_empty(env, GITHUB_TOKEN_ENV)
}

pub(crate) fn renovate_github_token_available<F>(env: &F) -> bool
where
    F: Fn(&str) -> Option<String>,
{
    env_non_empty(env, GITHUB_COM_TOKEN_ENV) || github_token_available(env)
}

pub(crate) fn token_warning(check_name: &str, token_names: &str) -> String {
    format!(
        "flint: warning: {token_names} is not set; {check_name} GitHub requests may be rate limited"
    )
}

fn env_truthy<F>(env: &F, name: &str) -> bool
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

        assert!(github_token_available(&|name| vars.get(name).cloned()));
    }

    #[test]
    fn detects_renovate_github_com_token() {
        let vars = HashMap::from([("GITHUB_COM_TOKEN".to_string(), "token".to_string())]);

        assert!(renovate_github_token_available(&|name| vars
            .get(name)
            .cloned()));
    }
}
