use std::collections::HashMap;

pub fn obsolete_keys() -> Vec<(&'static str, &'static str)> {
    crate::setup::obsolete_keys()
}

pub fn unsupported_keys() -> Vec<(&'static str, &'static str)> {
    crate::setup::unsupported_keys()
}

pub fn find_obsolete_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    crate::setup::find_obsolete_key(mise_tools)
}

pub fn find_unsupported_key(
    mise_tools: &HashMap<String, String>,
) -> Option<(&'static str, &'static str)> {
    crate::setup::find_unsupported_key(mise_tools)
}
