# Link remap smoke test

These links exercise the GitHub URL remap rules in `src/linters/lychee.rs`.
On PR branches, lychee rewrites `blob/main/` URLs to the PR branch —
these links verify that each remap rule works correctly during CI.

## Line-number anchors (`#L123`, `#L10-L20`) — fragment stripped, remapped to raw

- [README.md#L1](https://github.com/grafana/flint/blob/main/README.md#L1)
- [links.sh#L6](https://github.com/grafana/flint/blob/main/tasks/lint/links.sh#L6)
- [links.sh#L6-L10](https://github.com/grafana/flint/blob/main/tasks/lint/links.sh#L6-L10)

## Scroll to Text Fragment anchors (`#:~:text=...`) — fragment stripped, remapped to raw

<!-- editorconfig-checker-disable -->

- [links.sh text fragment](https://github.com/grafana/flint/blob/main/tasks/lint/links.sh#:~:text=build_remap_args)
<!-- editorconfig-checker-enable -->

## External Scroll to Text Fragment anchors — fragment stripped, remapped to raw

- [okhttp text fragment](https://github.com/square/okhttp/blob/master/README.md#:~:text=OkHttp)

## Section fragments (`#section`) — remapped to raw with fragment preserved

- [CHANGELOG.md heading](https://github.com/grafana/flint/blob/main/CHANGELOG.md#changelog)

## Non-fragment blob URLs — remapped to raw

- [LICENSE](https://github.com/grafana/flint/blob/main/LICENSE)

## Tree URLs — remapped to PR branch

- [tasks/lint directory](https://github.com/grafana/flint/tree/main/tasks/lint)

## External repository line-number anchors — fragment stripped, remapped to raw

These test the global remap that strips line-number anchors from ANY
GitHub repository (not just the current one). The file is remapped to
raw.githubusercontent.com and the JS-rendered fragment is skipped.

<!-- editorconfig-checker-disable -->

- [okhttp build.gradle#L144-L153](https://github.com/square/okhttp/blob/96a2118dd447ebc28a64d9b11a431ca642edc441/build.gradle#L144-L153)
<!-- editorconfig-checker-enable -->
- [lychee main.rs#L1](https://github.com/lycheeverse/lychee/blob/master/lychee-bin/src/main.rs#L1)

## Issue comment anchors — fragment stripped globally

Issue comment anchors are rendered by JavaScript and cannot be
verified by lychee. The fragment is stripped so the issue/PR page
is still checked.

- [example issue comment](https://github.com/grafana/flint/issues/1#issuecomment-1)
