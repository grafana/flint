use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use regex::{Captures, Regex};

#[cfg(test)]
use crate::config::{DerivedRegexReplaceRuleConfig, RegexReplaceIgnoreRegionConfig};
use crate::config::{RegexReplaceConfig, RegexReplaceRuleConfig, RegexReplaceSetConfig};
use crate::files::match_files;
use crate::linters::LinterOutput;
use crate::regions::{RegionSpan, find_region_spans};
use crate::registry::{
    CheckTypeDef, NativeCheckDef, NativePrepareContext, NativeRunContext, NativeRunFuture,
    PreparedNativeCheck, StatusContext,
};

pub(crate) static CHECK_TYPE: CheckTypeDef = CheckTypeDef::native(
    "regex-replace",
    NativeCheckDef::new(prepare)
        .with_fix()
        .with_config_display("via `[checks.regex-replace]` in flint.toml"),
);

#[derive(Debug)]
struct PreparedRegexReplace {
    name: String,
    cfg: RegexReplaceConfig,
    files: Vec<PathBuf>,
}

#[derive(Debug)]
struct CompiledConfig {
    sets: Vec<CompiledSet>,
}

#[derive(Debug)]
struct CompiledSet {
    name: String,
    patterns: Vec<String>,
    exclude: Vec<String>,
    replacement: Option<String>,
    rules: Vec<CompiledRule>,
    derived_rules: Vec<CompiledDerivedRule>,
    add_lines_before_pattern: Option<Regex>,
    add_lines_fallback_after_pattern: Option<Regex>,
    skip_line_pattern: Option<Regex>,
    ignore_regions: Vec<CompiledIgnoreRegion>,
}

#[derive(Debug)]
struct CompiledIgnoreRegion {
    start_pattern: Regex,
    end_pattern: Regex,
}

#[derive(Debug)]
struct CompiledRule {
    pattern: Regex,
    replacement: Option<String>,
    add_lines: Vec<String>,
    content_pattern: Option<Regex>,
    content_exclude_pattern: Option<Regex>,
    line_exclude_pattern: Option<Regex>,
    file_pattern: Option<Regex>,
}

#[derive(Debug)]
struct CompiledDerivedRule {
    source_pattern: Regex,
    pattern: String,
    replacement: Option<String>,
    add_lines: Vec<String>,
    source_exclude_pattern: Option<Regex>,
}

struct RewriteOptions<'a> {
    line_exclude: Option<&'a Regex>,
    skip_line: Option<&'a Regex>,
    ignore_regions: &'a [RegionSpan],
}

fn compile_config(cfg: &RegexReplaceConfig) -> Result<CompiledConfig, String> {
    let sets = cfg
        .sets
        .iter()
        .map(compile_set)
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CompiledConfig { sets })
}

fn compile_set(set: &RegexReplaceSetConfig) -> Result<CompiledSet, String> {
    let rules = set
        .rules
        .iter()
        .map(compile_rule)
        .collect::<Result<Vec<_>, String>>()?;
    let derived_rules = set
        .derived_rules
        .iter()
        .map(|rule| {
            Ok(CompiledDerivedRule {
                source_pattern: Regex::new(&rule.source_pattern).map_err(|error| {
                    format!("invalid source_pattern {:?}: {error}", rule.source_pattern)
                })?,
                pattern: rule.pattern.clone(),
                replacement: rule.replacement.clone(),
                add_lines: rule.add_lines.clone(),
                source_exclude_pattern: compile_optional(
                    rule.source_exclude_pattern.as_deref(),
                    "source_exclude_pattern",
                )?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let ignore_regions = set
        .ignore_regions
        .iter()
        .map(|region| {
            Ok(CompiledIgnoreRegion {
                start_pattern: Regex::new(&region.start_pattern).map_err(|error| {
                    format!(
                        "invalid ignore region start_pattern {:?}: {error}",
                        region.start_pattern
                    )
                })?,
                end_pattern: Regex::new(&region.end_pattern).map_err(|error| {
                    format!(
                        "invalid ignore region end_pattern {:?}: {error}",
                        region.end_pattern
                    )
                })?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(CompiledSet {
        name: set.name.clone(),
        patterns: set.patterns.clone(),
        exclude: set.exclude.clone(),
        replacement: set.replacement.clone(),
        rules,
        derived_rules,
        add_lines_before_pattern: compile_optional(
            set.add_lines_before_pattern.as_deref(),
            "add_lines_before_pattern",
        )?,
        add_lines_fallback_after_pattern: compile_optional(
            set.add_lines_fallback_after_pattern.as_deref(),
            "add_lines_fallback_after_pattern",
        )?,
        skip_line_pattern: compile_optional(set.skip_line_pattern.as_deref(), "skip_line_pattern")?,
        ignore_regions,
    })
}

fn compile_rule(rule: &RegexReplaceRuleConfig) -> Result<CompiledRule, String> {
    Ok(CompiledRule {
        pattern: Regex::new(&rule.pattern).map_err(|error| {
            format!("invalid regex-replace pattern {:?}: {error}", rule.pattern)
        })?,
        replacement: rule.replacement.clone(),
        add_lines: rule.add_lines.clone(),
        content_pattern: compile_optional(rule.content_pattern.as_deref(), "content_pattern")?,
        content_exclude_pattern: compile_optional(
            rule.content_exclude_pattern.as_deref(),
            "content_exclude_pattern",
        )?,
        line_exclude_pattern: compile_optional(
            rule.line_exclude_pattern.as_deref(),
            "line_exclude_pattern",
        )?,
        file_pattern: compile_optional(rule.file_pattern.as_deref(), "file_pattern")?,
    })
}

fn compile_optional(pattern: Option<&str>, name: &str) -> Result<Option<Regex>, String> {
    pattern
        .map(|pattern| Regex::new(pattern).map_err(|error| format!("invalid {name}: {error}")))
        .transpose()
}

fn prepare(ctx: NativePrepareContext<'_>) -> Option<Box<dyn PreparedNativeCheck>> {
    let cfg = &ctx.cfg.checks.regex_replace;
    if !is_configured(cfg) {
        return None;
    }

    let configured_patterns: Vec<&str> = cfg.patterns.iter().map(String::as_str).collect();
    let patterns = if configured_patterns.is_empty() {
        vec!["*"]
    } else {
        configured_patterns
    };
    let excludes: Vec<&str> = cfg.exclude.iter().map(String::as_str).collect();
    let files: Vec<PathBuf> =
        match_files(&ctx.file_list.files, &patterns, &excludes, ctx.project_root)
            .into_iter()
            .cloned()
            .collect();

    if files.is_empty() {
        return None;
    }

    Some(Box::new(PreparedRegexReplace {
        name: ctx.name.to_string(),
        cfg: cfg.clone(),
        files,
    }))
}

impl PreparedNativeCheck for PreparedRegexReplace {
    fn name(&self) -> &str {
        &self.name
    }

    fn tracked_files(&self) -> &[PathBuf] {
        &self.files
    }

    fn run(self: Box<Self>, ctx: NativeRunContext) -> NativeRunFuture {
        Box::pin(async move { run(&self.cfg, &ctx.project_root, &self.files, ctx.fix).await })
    }
}

pub(crate) fn status(ctx: &dyn StatusContext) -> Option<&'static str> {
    let cfg = &ctx.config().checks.regex_replace;
    (!is_configured(cfg)).then_some("not configured")
}

fn is_configured(cfg: &RegexReplaceConfig) -> bool {
    cfg.sets
        .iter()
        .any(|set| !set.rules.is_empty() || !set.derived_rules.is_empty())
}

pub(crate) async fn run(
    cfg: &RegexReplaceConfig,
    project_root: &Path,
    files: &[PathBuf],
    fix: bool,
) -> LinterOutput {
    let mut stderr = Vec::new();
    let mut all_ok = true;
    let compiled = match compile_config(cfg) {
        Ok(compiled) => compiled,
        Err(error) => {
            return LinterOutput::err(format!("regex-replace: {error}\n"));
        }
    };

    for file in files {
        let rel = file.strip_prefix(project_root).unwrap_or(file);
        let rel = rel.to_string_lossy();
        let input = match std::fs::read_to_string(file) {
            Ok(input) => input,
            Err(error) => {
                all_ok = false;
                stderr.extend_from_slice(
                    format!("{rel}: failed to read regex-replace input: {error}\n").as_bytes(),
                );
                continue;
            }
        };

        let output = match rewrite_compiled(&input, file, project_root, &compiled) {
            Ok(output) => output,
            Err(error) => {
                all_ok = false;
                stderr.extend_from_slice(format!("{rel}: {error}\n").as_bytes());
                continue;
            }
        };

        if output == input {
            continue;
        }

        if fix {
            if let Err(error) = std::fs::write(file, output) {
                all_ok = false;
                stderr.extend_from_slice(
                    format!("{rel}: failed to write regex-replace fix: {error}\n").as_bytes(),
                );
            }
        } else {
            all_ok = false;
            stderr.extend_from_slice(
                format!("{rel}: regex replacements are not formatted\n").as_bytes(),
            );
        }
    }

    LinterOutput {
        ok: all_ok,
        stdout: vec![],
        stderr,
        setup_outcome: None,
    }
}

#[cfg(test)]
fn rewrite(input: &str, file: &Path, cfg: &RegexReplaceConfig) -> Result<String, String> {
    let compiled = compile_config(cfg)?;
    rewrite_compiled(input, file, Path::new("."), &compiled)
}

fn rewrite_compiled(
    input: &str,
    file: &Path,
    project_root: &Path,
    cfg: &CompiledConfig,
) -> Result<String, String> {
    let mut content = input.to_string();

    for set in &cfg.sets {
        if !set_matches_file(set, file, project_root) {
            continue;
        }
        rewrite_set(&mut content, file, set)
            .map_err(|error| format!("rule set {:?}: {error}", set.name))?;
    }

    Ok(content)
}

fn set_matches_file(set: &CompiledSet, file: &Path, project_root: &Path) -> bool {
    if set.patterns.is_empty() && set.exclude.is_empty() {
        return true;
    }
    let configured_patterns: Vec<&str> = set.patterns.iter().map(String::as_str).collect();
    let patterns = if configured_patterns.is_empty() {
        vec!["*"]
    } else {
        configured_patterns
    };
    let excludes: Vec<&str> = set.exclude.iter().map(String::as_str).collect();
    !match_files(
        std::slice::from_ref(&file.to_path_buf()),
        &patterns,
        &excludes,
        project_root,
    )
    .is_empty()
}

fn rewrite_set(content: &mut String, file: &Path, set: &CompiledSet) -> Result<(), String> {
    let mut lines_to_add = BTreeSet::new();
    let lines: Vec<&str> = content.lines().collect();
    let ignored_regions = find_region_spans(
        &lines,
        &set.ignore_regions,
        |region, line| region.start_pattern.is_match(line),
        |region, line| region.end_pattern.is_match(line),
    )
    .map_err(|error| format!("invalid ignored region: {error}"))?;

    for rule in &set.rules {
        apply_rule(
            content,
            file,
            rule,
            set,
            &ignored_regions,
            &mut lines_to_add,
        );
    }
    for rule in &set.derived_rules {
        apply_derived_rule(content, rule, set, &ignored_regions, &mut lines_to_add)?;
    }

    *content = insert_lines(content, lines_to_add, set)?;
    Ok(())
}

fn apply_rule(
    content: &mut String,
    file: &Path,
    rule: &CompiledRule,
    set: &CompiledSet,
    ignored_regions: &[RegionSpan],
    lines_to_add: &mut BTreeSet<String>,
) {
    if let Some(pattern) = &rule.file_pattern {
        let file_name = file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if !pattern.is_match(file_name) {
            return;
        }
    }
    if let Some(pattern) = &rule.content_pattern
        && !pattern.is_match(content)
    {
        return;
    }
    if let Some(pattern) = &rule.content_exclude_pattern
        && pattern.is_match(content)
    {
        return;
    }

    let replacement = rule
        .replacement
        .as_deref()
        .or(set.replacement.as_deref())
        .unwrap_or("$0");
    rewrite_lines(
        content,
        &rule.pattern,
        replacement,
        &rule.add_lines,
        &RewriteOptions {
            line_exclude: rule.line_exclude_pattern.as_ref(),
            skip_line: set.skip_line_pattern.as_ref(),
            ignore_regions: ignored_regions,
        },
        lines_to_add,
    );
}

fn apply_derived_rule(
    content: &mut String,
    rule: &CompiledDerivedRule,
    set: &CompiledSet,
    ignored_regions: &[RegionSpan],
    lines_to_add: &mut BTreeSet<String>,
) -> Result<(), String> {
    let mut bindings = Vec::new();
    for (line_index, line) in content.lines().enumerate() {
        if ignored_regions
            .iter()
            .any(|region| region.contains(line_index))
        {
            continue;
        }
        let Some(captures) = rule.source_pattern.captures(line.trim()) else {
            continue;
        };
        if rule
            .source_exclude_pattern
            .as_ref()
            .is_some_and(|pattern| pattern.is_match(line.trim()))
        {
            continue;
        }
        let mut values = HashMap::new();
        for name in rule.source_pattern.capture_names().flatten() {
            if let Some(value) = captures.name(name) {
                values.insert(name.to_string(), value.as_str().to_string());
            }
        }
        bindings.push(values);
    }

    for values in bindings {
        let pattern = substitute_placeholders(&rule.pattern, &values, true);
        let regex = Regex::new(&pattern)
            .map_err(|error| format!("invalid derived pattern {pattern:?}: {error}"))?;
        let replacement_template = rule
            .replacement
            .as_deref()
            .or(set.replacement.as_deref())
            .unwrap_or("$0");
        let replacement = substitute_placeholders(replacement_template, &values, false);
        let add_lines: Vec<String> = rule
            .add_lines
            .iter()
            .map(|line| substitute_placeholders(line, &values, false))
            .collect();
        rewrite_lines(
            content,
            &regex,
            &replacement,
            &add_lines,
            &RewriteOptions {
                line_exclude: None,
                skip_line: set.skip_line_pattern.as_ref(),
                ignore_regions: ignored_regions,
            },
            lines_to_add,
        );
    }
    Ok(())
}

fn rewrite_lines(
    content: &mut String,
    regex: &Regex,
    replacement: &str,
    add_lines: &[String],
    options: &RewriteOptions<'_>,
    lines_to_add: &mut BTreeSet<String>,
) {
    let trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    for index in 0..lines.len() {
        if options
            .ignore_regions
            .iter()
            .any(|region| region.contains(index))
        {
            continue;
        }
        if options
            .skip_line
            .is_some_and(|pattern| pattern.is_match(&lines[index]))
        {
            continue;
        }
        if options.line_exclude.is_some_and(|pattern| {
            let start = index.saturating_sub(3);
            let context = lines[start..=index]
                .iter()
                .map(|line| line.trim())
                .collect::<Vec<_>>()
                .join(" ");
            pattern.is_match(&context)
        }) {
            continue;
        }

        let mut matched = false;
        let replaced = regex.replace_all(&lines[index], |captures: &Captures<'_>| {
            matched = true;
            for line in add_lines {
                let mut expanded = String::new();
                captures.expand(line, &mut expanded);
                lines_to_add.insert(expanded);
            }
            let mut expanded = String::new();
            captures.expand(replacement, &mut expanded);
            expanded
        });
        if matched {
            lines[index] = replaced.into_owned();
        }
    }

    *content = lines.join("\n");
    if trailing_newline {
        content.push('\n');
    }
}

fn substitute_placeholders(
    template: &str,
    values: &HashMap<String, String>,
    escape: bool,
) -> String {
    let mut output = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        output.push_str(&rest[..start]);
        let Some(end) = rest[start + 1..].find('}') else {
            output.push_str(&rest[start..]);
            break;
        };
        let end = start + 1 + end;
        let name = &rest[start + 1..end];
        if let Some(value) = values.get(name) {
            if escape {
                output.push_str(&regex::escape(value));
            } else {
                output.push_str(value);
            }
        } else {
            output.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    output.push_str(rest);
    output
}

fn insert_lines(
    content: &str,
    lines_to_add: BTreeSet<String>,
    set: &CompiledSet,
) -> Result<String, String> {
    if lines_to_add.is_empty() {
        return Ok(content.to_string());
    }
    let trailing_newline = content.ends_with('\n');
    let mut lines: Vec<String> = content.lines().map(ToString::to_string).collect();
    let existing: BTreeSet<&str> = lines.iter().map(String::as_str).collect();
    let additions: Vec<String> = lines_to_add
        .into_iter()
        .filter(|line| !existing.contains(line.as_str()))
        .collect();
    if additions.is_empty() {
        return Ok(content.to_string());
    }

    let insertion_index = set
        .add_lines_before_pattern
        .as_ref()
        .and_then(|pattern| lines.iter().position(|line| pattern.is_match(line)))
        .or_else(|| {
            set.add_lines_fallback_after_pattern
                .as_ref()
                .and_then(|pattern| lines.iter().position(|line| pattern.is_match(line)))
                .map(|index| index + 1)
        })
        .unwrap_or(0);
    for (offset, line) in additions.into_iter().enumerate() {
        lines.insert(insertion_index + offset, line);
    }

    let mut output = lines.join("\n");
    if trailing_newline {
        output.push('\n');
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(pattern: &str, add_line: &str) -> RegexReplaceRuleConfig {
        RegexReplaceRuleConfig {
            pattern: pattern.to_string(),
            replacement: None,
            add_lines: vec![add_line.to_string()],
            content_pattern: None,
            line_exclude_pattern: None,
            file_pattern: None,
            content_exclude_pattern: None,
        }
    }

    fn config(rule: RegexReplaceRuleConfig) -> RegexReplaceConfig {
        RegexReplaceConfig {
            patterns: vec![],
            exclude: vec![],
            sets: vec![RegexReplaceSetConfig {
                name: "test".to_string(),
                replacement: Some("$1".to_string()),
                rules: vec![rule],
                add_lines_before_pattern: Some("^use ".to_string()),
                skip_line_pattern: Some("^use ".to_string()),
                ..Default::default()
            }],
        }
    }

    #[test]
    fn rewrites_text_and_adds_a_line() {
        let input = "use foo.bar;\n\nvalue = foo.bar(value);\n";
        let output = rewrite(
            input,
            Path::new("example.txt"),
            &config(rule(r"\bfoo\.(bar)\b", "use static foo.$1;")),
        )
        .unwrap();
        assert_eq!(
            output,
            "use static foo.bar;\nuse foo.bar;\n\nvalue = bar(value);\n"
        );
    }

    #[test]
    fn honors_content_and_line_exclusions() {
        let mut configured = rule(r"\bfoo\.(bar)\b", "use static foo.$1;");
        configured.content_exclude_pattern = Some("skip-file".to_string());
        configured.line_exclude_pattern = Some("keep".to_string());
        let cfg = config(configured);
        let input =
            "use foo.bar;\n// keep foo.bar here\none\ntwo\nthree\nvalue = foo.bar(value);\n";
        let output = rewrite(input, Path::new("example.txt"), &cfg).unwrap();
        assert!(output.contains("// keep foo.bar here"));
        assert!(output.contains("value = bar(value);"));
    }

    #[test]
    fn ignores_configured_regions() {
        let mut cfg = config(rule(r"\bfoo\.(bar)\b", "use static foo.$1;"));
        cfg.sets[0].ignore_regions = vec![RegexReplaceIgnoreRegionConfig {
            start_pattern: "^BEGIN$".to_string(),
            end_pattern: "^END$".to_string(),
        }];
        let input = "BEGIN\nvalue = foo.bar(value);\nEND\nvalue = foo.bar(value);\n";
        let output = rewrite(input, Path::new("example.txt"), &cfg).unwrap();
        assert_eq!(
            output,
            "use static foo.bar;\nBEGIN\nvalue = foo.bar(value);\nEND\nvalue = bar(value);\n"
        );
    }

    #[test]
    fn derives_rules_from_named_source_captures() {
        let cfg = RegexReplaceConfig {
            patterns: vec![],
            exclude: vec![],
            sets: vec![RegexReplaceSetConfig {
                name: "derived".to_string(),
                replacement: Some("$1".to_string()),
                derived_rules: vec![DerivedRegexReplaceRuleConfig {
                    source_pattern: r"^use (?P<module>[a-z.]+)\.(?P<name>[A-Z][A-Za-z0-9]+);$"
                        .to_string(),
                    pattern: r"\b{name}\.([A-Z_]+)\b".to_string(),
                    replacement: None,
                    add_lines: vec!["use static {module}.$1;".to_string()],
                    source_exclude_pattern: None,
                }],
                add_lines_before_pattern: Some("^use ".to_string()),
                skip_line_pattern: Some(r"^use ".to_string()),
                ..Default::default()
            }],
        };
        let input = "use example.mod.Constants;\n\nvalue = Constants.VALUE;\n";
        let output = rewrite(input, Path::new("example.txt"), &cfg).unwrap();
        assert_eq!(
            output,
            "use static example.mod.VALUE;\nuse example.mod.Constants;\n\nvalue = VALUE;\n"
        );
    }
}
