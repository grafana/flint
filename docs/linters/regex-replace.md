# `regex-replace`

`regex-replace` is Flint's built-in, line-oriented rewrite engine. It applies
regular-expression rules to selected files and can add text when a rule
matches. It does not parse a programming language or contain Java-specific
logic; the language convention is expressed by the configured regular
expressions.

This makes it useful for repository-specific mechanical changes that do not
belong in a general-purpose formatter.

## Example: qualified Java references to static imports

One example is replacing frequently used qualified Java references with static
imports. Given:

```java
import java.util.Objects;

class Example {
  void check(Object value) {
    Objects.requireNonNull(value);
  }
}
```

the desired result is:

```java
import static java.util.Objects.requireNonNull;
import java.util.Objects;

class Example {
  void check(Object value) {
    requireNonNull(value);
  }
}
```

The rule has two effects:

1. It rewrites `Objects.requireNonNull` to `requireNonNull`.
2. It adds the corresponding import once, before the first import or after
   the package declaration when the file has no imports.

Here is a realistic configuration based on the static-import migration this
check was designed to support:

```toml
[checks.regex-replace]
patterns = ["*.java"]
exclude = ["generated/**", "build/**"]

[[checks.regex-replace.sets]]
name = "static-imports"

# Rules inherit this replacement unless they specify one themselves.
# $1 is the first capture group in each direct rule.
replacement = '$1'
skip_line_pattern = '^\s*import '
add_lines_before_pattern = '^\s*import '
add_lines_fallback_after_pattern = '^\s*package '

# Do not rewrite examples or references inside block comments.
ignore_regions = [
  { start_pattern = '^\s*/\*', end_pattern = '\*/' },
]

[[checks.regex-replace.sets.rules]]
pattern = '\bObjects\.(requireNonNull)\b'
add_lines = ['import static java.util.Objects.$1;']

[[checks.regex-replace.sets.rules]]
pattern = '\bElementMatchers\.([a-z][a-zA-Z0-9]*)\b'
add_lines = ['import static net.bytebuddy.matcher.ElementMatchers.$1;']

[[checks.regex-replace.sets.rules]]
pattern = '\bMockito\.(mock|mockStatic|spy|when|verify|never|times)\b'
add_lines = ['import static org.mockito.Mockito.$1;']

[[checks.regex-replace.sets.rules]]
pattern = '(^|[^.])\bLevel\.([A-Z][A-Z_0-9]*)\b'
replacement = '$1$2'
add_lines = ['import static java.util.logging.Level.$2;']
content_pattern = '(?m)^\s*import java\.util\.logging\.Level;$'

[[checks.regex-replace.sets.rules]]
pattern = '\bAttributeKey\.(stringKey|longKey|booleanKey|doubleKey)\b'
add_lines = ['import static io.opentelemetry.api.common.AttributeKey.$1;']
line_exclude_pattern = '= AttributeKey\.'
file_pattern = 'Test\.java$'

# Generate rules from existing imports. For example, an import of
# io.opentelemetry.semconv.http.HttpAttributes lets the same rule rewrite
# HttpAttributes.HTTP_REQUEST_METHOD and add the matching static import.
[[checks.regex-replace.sets.derived_rules]]
source_pattern = '^import (?P<package>io\.opentelemetry\.semconv(?:\.[a-z][a-z.]*)?\.)(?P<class>[A-Z][a-zA-Z0-9]+);$'
pattern = '\b{class}\.([A-Z][A-Z_0-9]*)\b'
add_lines = ['import static {package}$1;']
source_exclude_pattern = 'SchemaUrls'
```

Run it like any other fixable Flint check:

```bash
flint run regex-replace
flint run --fix regex-replace
```

## How the configuration is organized

### File selection

The check-level `patterns` and `exclude` select the files Flint gives to the
linter. Each rule set can add its own `patterns` and `exclude`, using the same
glob syntax as `[settings].exclude`:

```toml
[checks.regex-replace]
patterns = ["*.java", "*.kt"]
exclude = ["generated/**"]

[[checks.regex-replace.sets]]
name = "java-only"
patterns = ["*.java"]
exclude = ["examples/**"]
```

This lets one `regex-replace` check contain independent rule sets for
different file types or directory scopes.

### Rule sets and defaults

Sets are evaluated in order. A set groups rules that share policy and defaults:

- `replacement` is inherited by direct and derived rules.
- A rule-level `replacement` overrides the set default.
- If neither is configured, the replacement is `$0`, preserving the match.
- `add_lines_before_pattern` and
  `add_lines_fallback_after_pattern` control where unique added lines go.
- `skip_line_pattern` skips entire lines for every rule in the set.
- `ignore_regions` skips balanced regions for every rule in the set.

Added lines are deduplicated against the file, so rerunning the fixer does not
keep adding the same import.

### Rule filters

Direct rules support additional filters:

- `content_pattern` — only apply when the whole file contains a match.
- `content_exclude_pattern` — skip the rule when the whole file contains a
  match.
- `line_exclude_pattern` — skip a line when its nearby context matches.
- `file_pattern` — restrict the rule using the file name.

Derived rules use `source_pattern` to find source lines and named captures such
as `{package}` and `{class}` to generate a rule. Their
`source_exclude_pattern` prevents selected source lines from generating rules.
Capture groups from the generated rule remain available as `$1`, `$2`, and so
on in `replacement` and `add_lines`.

### Ignored regions

`ignore_regions` is line-based and generic. Each `start_pattern` and
`end_pattern` is a regular expression matched against a complete source line;
the marker lines and every line between them are skipped. The markers must be
balanced. This is useful for block comments, generated snippets, or repository
conventions that should not be rewritten.

It is deliberately separate from formatter-specific formatter-off handling:
`regex-replace` skips those lines, while a formatter integration may instead
format a temporary copy and restore the protected contents afterward.

## Limitations

`regex-replace` is intentionally not a parser or import sorter. It cannot
prove that a replacement is syntactically valid, resolve overloaded methods,
or determine whether a static import conflicts with another symbol. Keep the
patterns narrow, use content and file filters where needed, and review the
result of a new rule with `flint run --fix` before enabling it broadly.
