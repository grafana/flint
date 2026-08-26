//! Fix eligibility and configuration helpers for `flint run`.

use crate::registry;
use crate::registry::{CheckKind, FixBehavior, LinterConfig, Scope};

pub(crate) fn canonical_config_path(config: &LinterConfig) -> String {
    config.canonical_location()
}

pub(crate) fn is_fixable(name: &str, active: &[&registry::Check]) -> bool {
    name == "flint-setup" || active.iter().any(|c| c.name == name && c.has_fix())
}

pub(crate) fn supports_single_pass_fix(check: &registry::Check) -> bool {
    check.has_fix()
        && check.fix_behavior() == FixBehavior::Definitive
        && matches!(
            check.kind,
            CheckKind::Template {
                scope: Scope::File | Scope::Files,
                ..
            }
        )
}
