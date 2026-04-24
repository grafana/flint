# Changelog

## [0.21.0](https://github.com/grafana/flint/compare/v0.20.4...v0.21.0) - 2026-04-24

### Added

- add taplo formatter check ([#224](https://github.com/grafana/flint/pull/224))
- *(init)* configure line length during init ([#218](https://github.com/grafana/flint/pull/218))

### Fixed

- switch yaml-lint to ryl ([#226](https://github.com/grafana/flint/pull/226))
- *(ruff)* install via github releases ([#227](https://github.com/grafana/flint/pull/227))
- resolve init rollout regressions ([#220](https://github.com/grafana/flint/pull/220))

### Other

- *(deps)* update taiki-e/install-action digest to 787505c ([#219](https://github.com/grafana/flint/pull/219))
- *(deps)* update dependency grafana/flint to v0.20.3 ([#225](https://github.com/grafana/flint/pull/225))

## [0.20.4](https://github.com/grafana/flint/compare/v0.20.3...v0.20.4) - 2026-04-23

### Added

- group linter renovate updates ([#209](https://github.com/grafana/flint/pull/209))

### Fixed

- expand baseline guards for config and flint changes ([#215](https://github.com/grafana/flint/pull/215))
- remove stale exclude_paths init placeholder ([#211](https://github.com/grafana/flint/pull/211))
- baseline lint coverage changes ([#214](https://github.com/grafana/flint/pull/214))
- align biome init and formatter ownership ([#205](https://github.com/grafana/flint/pull/205))

### Other

- *(deps)* update dependency npm:renovate to v43.133.0 ([#216](https://github.com/grafana/flint/pull/216))
- *(deps)* update dependency npm:renovate to v43.132.2 ([#212](https://github.com/grafana/flint/pull/212))
- *(deps)* update taiki-e/install-action digest to 5f57d6c ([#204](https://github.com/grafana/flint/pull/204))
- clarify that flint init works with existing mise.toml ([#208](https://github.com/grafana/flint/pull/208))
- guard against overlapping fixer ownership ([#206](https://github.com/grafana/flint/pull/206))

## [0.20.3](https://github.com/grafana/flint/compare/v0.20.2...v0.20.3) - 2026-04-21

### Added

- *(registry)* switch shfmt to aqua backend ([#175](https://github.com/grafana/flint/pull/175))

### Fixed

- treat cargo-clippy as a partial fixer ([#197](https://github.com/grafana/flint/pull/197))
- *(registry)* add --tests to cargo-clippy, add test coverage ([#176](https://github.com/grafana/flint/pull/176))

### Other

- *(deps)* update taiki-e/install-action digest to 055f5df ([#180](https://github.com/grafana/flint/pull/180))
- *(deps)* update dependency npm:@biomejs/biome to v2.4.12 ([#191](https://github.com/grafana/flint/pull/191))
- *(deps)* update rust crate clap to v4.6.1 ([#196](https://github.com/grafana/flint/pull/196))
- *(deps)* update rust crate tokio to v1.52.1 ([#192](https://github.com/grafana/flint/pull/192))
- *(deps)* update dependency pipx:ruff to v0.15.11 ([#198](https://github.com/grafana/flint/pull/198))
- *(deps)* update node.js to v24.15.0 ([#194](https://github.com/grafana/flint/pull/194))
- *(deps)* update dependency npm:prettier to v3.8.3 ([#193](https://github.com/grafana/flint/pull/193))
- exclude mise install dir from Windows Defender ([#188](https://github.com/grafana/flint/pull/188))
- *(deps)* update dependency npm:renovate to v43.129.0 ([#200](https://github.com/grafana/flint/pull/200))
- restructure README/docs and split registry module ([#187](https://github.com/grafana/flint/pull/187))
- *(deps)* update dependency mise to v2026.4.15 ([#199](https://github.com/grafana/flint/pull/199))

## [0.20.2](https://github.com/grafana/flint/compare/v0.20.1...v0.20.2) - 2026-04-17

### Fixed

- *(release)* run release-pr after release to avoid race ([#184](https://github.com/grafana/flint/pull/184))

## [0.20.1](https://github.com/grafana/flint/compare/v0.20.0...v0.20.1) - 2026-04-16

### Added

- *(release)* migrate from release-please to release-plz ([#171](https://github.com/grafana/flint/pull/171))

### Fixed

- *(release)* use correct template variable in pr_body ([#178](https://github.com/grafana/flint/pull/178))
- *(release)* suppress component prefix in release-please tags ([#166](https://github.com/grafana/flint/pull/166))
- *(release)* add workflow_dispatch to retrigger for existing tags ([#167](https://github.com/grafana/flint/pull/167))

### Other

- move icon to assets/ to fix release-plz ([#177](https://github.com/grafana/flint/pull/177))
- *(deps)* update dependency npm:renovate to v43.102.11 [security] ([#174](https://github.com/grafana/flint/pull/174))
- *(deps)* update rust crate similar to v3.1.0 ([#173](https://github.com/grafana/flint/pull/173))
- *(deps)* update dependency github:mvdan/sh to v3.13.1 ([#163](https://github.com/grafana/flint/pull/163))

## [0.20.0](https://github.com/grafana/flint/compare/flint-v0.19.0...flint-v0.20.0) (2026-04-13)


### Features

* add flint v2 Rust binary ([#139](https://github.com/grafana/flint/issues/139)) ([19f2b25](https://github.com/grafana/flint/commit/19f2b2527b4420956f2f3f6b35cc946159370db5))
* add native linting mode and version mapping infrastructure ([#93](https://github.com/grafana/flint/issues/93)) ([24b06da](https://github.com/grafana/flint/commit/24b06da3eeeb97722cf280b5815c75c3ec31f134))
* add Renovate shareable preset for consuming repos ([#17](https://github.com/grafana/flint/issues/17)) ([8a06590](https://github.com/grafana/flint/commit/8a06590741fc0db0a801a84337928d5388e22d1a))
* consolidate link checking and add autofix flags ([#7](https://github.com/grafana/flint/issues/7)) ([086a5e9](https://github.com/grafana/flint/commit/086a5e9c942a373a276e6a77853066187ed2c268))
* flint update command, explicit JAR flag, v0.20.0 ([#146](https://github.com/grafana/flint/issues/146)) ([b43bf52](https://github.com/grafana/flint/commit/b43bf523b5ee6944ce67f7082ea4b4aea496e9ea))
* handle line-number anchors and issue comments globally ([#56](https://github.com/grafana/flint/issues/56)) ([cf751df](https://github.com/grafana/flint/commit/cf751df1093a06d8dfac60c9918ef129944494e4))
* **links:** add GitHub URL remaps for line-number and fragment anchors ([#28](https://github.com/grafana/flint/issues/28)) ([5b59065](https://github.com/grafana/flint/commit/5b590653fbd24963fba8e99e409b9977ec2410fc))
* **links:** auto-remap base-branch GitHub URLs to PR branch ([#18](https://github.com/grafana/flint/issues/18)) ([dd6cc61](https://github.com/grafana/flint/commit/dd6cc616792680be71ca364d37150a90644db3d4))
* **renovate:** support SHA-pinned URLs in Renovate preset ([#21](https://github.com/grafana/flint/issues/21)) ([4fd1f28](https://github.com/grafana/flint/commit/4fd1f28c2ced164e15c8663e5fc3ac28f9217ca8))
* **super-linter:** default to slim image ([#24](https://github.com/grafana/flint/issues/24)) ([c8eeab8](https://github.com/grafana/flint/commit/c8eeab82e5db39f0cf8b57a5ee7ac1fc7106a1b0))
* support NATIVE env var for container-free linting ([#107](https://github.com/grafana/flint/issues/107)) ([0a8193d](https://github.com/grafana/flint/commit/0a8193d5b0c264430b7a78c56a0fe0418173ff37))


### Bug Fixes

* activate mise environment in native lint mode ([#123](https://github.com/grafana/flint/issues/123)) ([d0fec45](https://github.com/grafana/flint/commit/d0fec4574c1905efb22e7e75bcca7ba7c2db64cf))
* add 'mise run fix' hint to lint failure output ([#90](https://github.com/grafana/flint/issues/90)) ([5b4ad5d](https://github.com/grafana/flint/commit/5b4ad5d2a2fc53e0d11de924b183adb4fb4f5a90))
* decouple version mapping generation from pinned super-linter version ([#112](https://github.com/grafana/flint/issues/112)) ([5370e77](https://github.com/grafana/flint/commit/5370e77a864084c146502f9c265792d035517376))
* **deps:** update rust crate crossterm to 0.29 ([#156](https://github.com/grafana/flint/issues/156)) ([c59ae3e](https://github.com/grafana/flint/commit/c59ae3ea3da34782eaa1eeb8faba8552151f558d))
* **deps:** update rust crate similar to v3 ([#160](https://github.com/grafana/flint/issues/160)) ([684be4e](https://github.com/grafana/flint/commit/684be4e2a1f26da34ab6a2d35dbb0a5369747596))
* **deps:** update rust crate toml to v1 ([#161](https://github.com/grafana/flint/issues/161)) ([3aae614](https://github.com/grafana/flint/commit/3aae614582b59b0c46bed37b411bdb2753dcee5f))
* **deps:** update rust crate toml_edit to 0.25 ([#158](https://github.com/grafana/flint/issues/158)) ([42d9efd](https://github.com/grafana/flint/commit/42d9efded7507704e5684bf7c1f06dd4ff667740))
* exclude GitHub compare links from lychee checks ([#10](https://github.com/grafana/flint/issues/10)) ([e714608](https://github.com/grafana/flint/commit/e714608d1d7550c5540f9e79cb6f14c9ed86a5ad))
* fail native lint when enabled tools are missing ([#111](https://github.com/grafana/flint/issues/111)) ([163bb6b](https://github.com/grafana/flint/commit/163bb6b31e558af4a977c94cf8489311a085fc54))
* improve link checker reliability against GitHub rate limiting ([#95](https://github.com/grafana/flint/issues/95)) ([7a5282d](https://github.com/grafana/flint/commit/7a5282de91df8a67dad3fd6ac8fc2b082434d8df))
* include staged files in native lint file list ([#135](https://github.com/grafana/flint/issues/135)) ([34412d6](https://github.com/grafana/flint/commit/34412d69d9af0b189dbe8de5a5549a964d8cfe80))
* **links:** add regex anchors to remap patterns ([#19](https://github.com/grafana/flint/issues/19)) ([2e17348](https://github.com/grafana/flint/commit/2e1734890548f9694c768a87d650f3f80a253f89))
* native lint in worktrees, trust toml, use ec binary, drop isort ([#134](https://github.com/grafana/flint/issues/134)) ([8594bba](https://github.com/grafana/flint/commit/8594bbabd4de528da476d0f6ead9dfb49913dd8a))
* **release-please:** fix footer not appearing on release PRs ([#40](https://github.com/grafana/flint/issues/40)) ([d7a55e4](https://github.com/grafana/flint/commit/d7a55e4a2ea3754afb84c5c31eeb742b374c21e0))
* remap same-repo GitHub URLs to local file paths ([#100](https://github.com/grafana/flint/issues/100)) ([b4feadd](https://github.com/grafana/flint/commit/b4feaddd9af690574eaefc112f465b556bf9c345))
* **renovate-deps:** forward GITHUB_TOKEN as GITHUB_COM_TOKEN ([#132](https://github.com/grafana/flint/issues/132)) ([4d6510b](https://github.com/grafana/flint/commit/4d6510b78361f2fe0bfd1d0cfe27ce8e26256054))
* replace broken release-please PR comment with docs ([#12](https://github.com/grafana/flint/issues/12)) ([817b37d](https://github.com/grafana/flint/commit/817b37df94fcd6be43fa61594d94e9988d3c6c8d))
* run shellcheck on .bats files in native mode ([#137](https://github.com/grafana/flint/issues/137)) ([a4fd3f8](https://github.com/grafana/flint/commit/a4fd3f8ea41d9b155b13805336549e2dcad49bd4))
* strip Scroll to Text Fragment anchors in link checks ([#86](https://github.com/grafana/flint/issues/86)) ([b630cdf](https://github.com/grafana/flint/commit/b630cdfdd53c67f2e1f744ff89787fe18342e389))
* tighten markdownlint config for native mode ([#106](https://github.com/grafana/flint/issues/106)) ([6ef25b2](https://github.com/grafana/flint/commit/6ef25b2fd3f3887e4be9918317526ddc77b65575))
* use remap instead of exclude for issue comment anchors ([#58](https://github.com/grafana/flint/issues/58)) ([656f355](https://github.com/grafana/flint/commit/656f355db5280a86c885da957f34193d1efc800e))

## [0.9.2](https://github.com/grafana/flint/compare/v0.9.1...v0.9.2) (2026-03-31)


### Bug Fixes

* include staged files in native lint file list ([#135](https://github.com/grafana/flint/issues/135)) ([34412d6](https://github.com/grafana/flint/commit/34412d69d9af0b189dbe8de5a5549a964d8cfe80))
* native lint in worktrees, trust toml, use ec binary, drop isort ([#134](https://github.com/grafana/flint/issues/134)) ([8594bba](https://github.com/grafana/flint/commit/8594bbabd4de528da476d0f6ead9dfb49913dd8a))
* **renovate-deps:** forward GITHUB_TOKEN as GITHUB_COM_TOKEN ([#132](https://github.com/grafana/flint/issues/132)) ([4d6510b](https://github.com/grafana/flint/commit/4d6510b78361f2fe0bfd1d0cfe27ce8e26256054))

## [0.9.1](https://github.com/grafana/flint/compare/v0.9.0...v0.9.1) (2026-03-19)


### Bug Fixes

* activate mise environment in native lint mode ([#123](https://github.com/grafana/flint/issues/123)) ([d0fec45](https://github.com/grafana/flint/commit/d0fec4574c1905efb22e7e75bcca7ba7c2db64cf))

## [0.9.0](https://github.com/grafana/flint/compare/v0.8.0...v0.9.0) (2026-03-19)


### Features

* support NATIVE env var for container-free linting ([#107](https://github.com/grafana/flint/issues/107)) ([0a8193d](https://github.com/grafana/flint/commit/0a8193d5b0c264430b7a78c56a0fe0418173ff37))


### Bug Fixes

* decouple version mapping generation from pinned super-linter version ([#112](https://github.com/grafana/flint/issues/112)) ([5370e77](https://github.com/grafana/flint/commit/5370e77a864084c146502f9c265792d035517376))
* fail native lint when enabled tools are missing ([#111](https://github.com/grafana/flint/issues/111)) ([163bb6b](https://github.com/grafana/flint/commit/163bb6b31e558af4a977c94cf8489311a085fc54))
* tighten markdownlint config for native mode ([#106](https://github.com/grafana/flint/issues/106)) ([6ef25b2](https://github.com/grafana/flint/commit/6ef25b2fd3f3887e4be9918317526ddc77b65575))

## [0.8.0](https://github.com/grafana/flint/compare/v0.7.1...v0.8.0) (2026-03-11)


### Features

* add native linting mode and version mapping infrastructure ([#93](https://github.com/grafana/flint/issues/93)) ([24b06da](https://github.com/grafana/flint/commit/24b06da3eeeb97722cf280b5815c75c3ec31f134))


### Bug Fixes

* add 'mise run fix' hint to lint failure output ([#90](https://github.com/grafana/flint/issues/90)) ([5b4ad5d](https://github.com/grafana/flint/commit/5b4ad5d2a2fc53e0d11de924b183adb4fb4f5a90))
* improve link checker reliability against GitHub rate limiting ([#95](https://github.com/grafana/flint/issues/95)) ([7a5282d](https://github.com/grafana/flint/commit/7a5282de91df8a67dad3fd6ac8fc2b082434d8df))
* remap same-repo GitHub URLs to local file paths ([#100](https://github.com/grafana/flint/issues/100)) ([b4feadd](https://github.com/grafana/flint/commit/b4feaddd9af690574eaefc112f465b556bf9c345))

## [0.7.1](https://github.com/grafana/flint/compare/v0.7.0...v0.7.1) (2026-03-02)


### Bug Fixes

* strip Scroll to Text Fragment anchors in link checks ([#86](https://github.com/grafana/flint/issues/86)) ([b630cdf](https://github.com/grafana/flint/commit/b630cdfdd53c67f2e1f744ff89787fe18342e389))

## [0.7.0](https://github.com/grafana/flint/compare/v0.6.0...v0.7.0) (2026-02-23)


### Features

* handle line-number anchors and issue comments globally ([#56](https://github.com/grafana/flint/issues/56)) ([cf751df](https://github.com/grafana/flint/commit/cf751df1093a06d8dfac60c9918ef129944494e4))


### Bug Fixes

* **release-please:** fix footer not appearing on release PRs ([#40](https://github.com/grafana/flint/issues/40)) ([d7a55e4](https://github.com/grafana/flint/commit/d7a55e4a2ea3754afb84c5c31eeb742b374c21e0))
* use remap instead of exclude for issue comment anchors ([#58](https://github.com/grafana/flint/issues/58)) ([656f355](https://github.com/grafana/flint/commit/656f355db5280a86c885da957f34193d1efc800e))

## [0.6.0](https://github.com/grafana/flint/compare/v0.5.0...v0.6.0) (2026-02-18)


### Features

* **links:** add GitHub URL remaps for line-number and fragment anchors ([#28](https://github.com/grafana/flint/issues/28)) ([5b59065](https://github.com/grafana/flint/commit/5b590653fbd24963fba8e99e409b9977ec2410fc))

## [0.5.0](https://github.com/grafana/flint/compare/v0.4.0...v0.5.0) (2026-02-17)


### Features

* **super-linter:** default to slim image ([#24](https://github.com/grafana/flint/issues/24)) ([c8eeab8](https://github.com/grafana/flint/commit/c8eeab82e5db39f0cf8b57a5ee7ac1fc7106a1b0))

## [0.4.0](https://github.com/grafana/flint/compare/v0.3.0...v0.4.0) (2026-02-16)


### Features

* **renovate:** support SHA-pinned URLs in Renovate preset ([#21](https://github.com/grafana/flint/issues/21)) ([4fd1f28](https://github.com/grafana/flint/commit/4fd1f28c2ced164e15c8663e5fc3ac28f9217ca8))

## [0.3.0](https://github.com/grafana/flint/compare/v0.2.0...v0.3.0) (2026-02-16)


### Features

* add Renovate shareable preset for consuming repos ([#17](https://github.com/grafana/flint/issues/17)) ([8a06590](https://github.com/grafana/flint/commit/8a06590741fc0db0a801a84337928d5388e22d1a))
* **links:** auto-remap base-branch GitHub URLs to PR branch ([#18](https://github.com/grafana/flint/issues/18)) ([dd6cc61](https://github.com/grafana/flint/commit/dd6cc616792680be71ca364d37150a90644db3d4))


### Bug Fixes

* **links:** add regex anchors to remap patterns ([#19](https://github.com/grafana/flint/issues/19)) ([2e17348](https://github.com/grafana/flint/commit/2e1734890548f9694c768a87d650f3f80a253f89))
* replace broken release-please PR comment with docs ([#12](https://github.com/grafana/flint/issues/12)) ([817b37d](https://github.com/grafana/flint/commit/817b37df94fcd6be43fa61594d94e9988d3c6c8d))

## [0.2.0](https://github.com/grafana/flint/compare/v0.1.0...v0.2.0) (2026-02-16)


### Features

* consolidate link checking and add autofix flags ([#7](https://github.com/grafana/flint/issues/7)) ([086a5e9](https://github.com/grafana/flint/commit/086a5e9c942a373a276e6a77853066187ed2c268))


### Bug Fixes

* exclude GitHub compare links from lychee checks ([#10](https://github.com/grafana/flint/issues/10)) ([e714608](https://github.com/grafana/flint/commit/e714608d1d7550c5540f9e79cb6f14c9ed86a5ad))

## Changelog
