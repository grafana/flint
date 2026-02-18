# Link remap smoke test

These links exercise the GitHub URL remap rules in `tasks/lint/links.sh`.
On PR branches, lychee rewrites `blob/main/` URLs to the PR branch —
these links verify that each remap rule works correctly during CI.

## Line-number anchors (`#L123`) — fragment stripped, file checked on PR branch

- [README.md#L1](https://github.com/grafana/flint/blob/main/README.md#L1)
- [links.sh#L6](https://github.com/grafana/flint/blob/main/tasks/lint/links.sh#L6)

## Section fragments (`#section`) — remapped to raw.githubusercontent.com

- [CHANGELOG.md heading](https://github.com/grafana/flint/blob/main/CHANGELOG.md#changelog)

## Non-fragment blob URLs — remapped to PR branch

- [LICENSE](https://github.com/grafana/flint/blob/main/LICENSE)

## Tree URLs — remapped to PR branch

- [tasks/lint directory](https://github.com/grafana/flint/tree/main/tasks/lint)
