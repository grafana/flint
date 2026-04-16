# Changelog

## [0.20.2](https://github.com/grafana/flint/compare/v0.20.1...v0.20.2) - 2026-04-16

### Added

- *(release)* migrate from release-please to release-plz ([#171](https://github.com/grafana/flint/pull/171))
- flint update command, explicit JAR flag, v0.20.0 ([#146](https://github.com/grafana/flint/pull/146))
- add flint v2 Rust binary ([#139](https://github.com/grafana/flint/pull/139))
- support NATIVE env var for container-free linting ([#107](https://github.com/grafana/flint/pull/107))
- add native linting mode and version mapping infrastructure ([#93](https://github.com/grafana/flint/pull/93))
- handle line-number anchors and issue comments globally ([#56](https://github.com/grafana/flint/pull/56))
- *(links)* add GitHub URL remaps for line-number and fragment anchors ([#28](https://github.com/grafana/flint/pull/28))
- *(super-linter)* default to slim image ([#24](https://github.com/grafana/flint/pull/24))
- *(links)* auto-remap base-branch GitHub URLs to PR branch ([#18](https://github.com/grafana/flint/pull/18))
- add Renovate shareable preset for consuming repos ([#17](https://github.com/grafana/flint/pull/17))

### Fixed

- *(release)* disable crates.io publishing ([#182](https://github.com/grafana/flint/pull/182))
- *(release)* use correct template variable in pr_body ([#178](https://github.com/grafana/flint/pull/178))
- *(release)* suppress component prefix in release-please tags ([#166](https://github.com/grafana/flint/pull/166))
- *(release)* add workflow_dispatch to retrigger for existing tags ([#167](https://github.com/grafana/flint/pull/167))
- *(deps)* update rust crate toml_edit to 0.25 ([#158](https://github.com/grafana/flint/pull/158))
- *(deps)* update rust crate toml to v1 ([#161](https://github.com/grafana/flint/pull/161))
- *(deps)* update rust crate similar to v3 ([#160](https://github.com/grafana/flint/pull/160))
- *(deps)* update rust crate crossterm to 0.29 ([#156](https://github.com/grafana/flint/pull/156))
- run shellcheck on .bats files in native mode ([#137](https://github.com/grafana/flint/pull/137))
- include staged files in native lint file list ([#135](https://github.com/grafana/flint/pull/135))
- native lint in worktrees, trust toml, use ec binary, drop isort ([#134](https://github.com/grafana/flint/pull/134))
- *(renovate-deps)* forward GITHUB_TOKEN as GITHUB_COM_TOKEN ([#132](https://github.com/grafana/flint/pull/132))
- activate mise environment in native lint mode ([#123](https://github.com/grafana/flint/pull/123))
- tighten markdownlint config for native mode ([#106](https://github.com/grafana/flint/pull/106))
- decouple version mapping generation from pinned super-linter version ([#112](https://github.com/grafana/flint/pull/112))
- fail native lint when enabled tools are missing ([#111](https://github.com/grafana/flint/pull/111))
- remap same-repo GitHub URLs to local file paths ([#100](https://github.com/grafana/flint/pull/100))
- add 'mise run fix' hint to lint failure output ([#90](https://github.com/grafana/flint/pull/90))
- improve link checker reliability against GitHub rate limiting ([#95](https://github.com/grafana/flint/pull/95))
- strip Scroll to Text Fragment anchors in link checks ([#86](https://github.com/grafana/flint/pull/86))
- use remap instead of exclude for issue comment anchors ([#58](https://github.com/grafana/flint/pull/58))
- *(release-please)* fix footer not appearing on release PRs ([#40](https://github.com/grafana/flint/pull/40))
- *(links)* add regex anchors to remap patterns ([#19](https://github.com/grafana/flint/pull/19))
- replace broken release-please PR comment with docs ([#12](https://github.com/grafana/flint/pull/12))
- exclude GitHub compare links from lychee checks ([#10](https://github.com/grafana/flint/pull/10))

### Other

- release v0.20.1 ([#179](https://github.com/grafana/flint/pull/179))
- move icon to assets/ to fix release-plz ([#177](https://github.com/grafana/flint/pull/177))
- *(deps)* update dependency npm:renovate to v43.102.11 [security] ([#174](https://github.com/grafana/flint/pull/174))
- *(deps)* update rust crate similar to v3.1.0 ([#173](https://github.com/grafana/flint/pull/173))
- *(deps)* update dependency github:mvdan/sh to v3.13.1 ([#163](https://github.com/grafana/flint/pull/163))
- *(main)* release flint 0.20.0 ([#165](https://github.com/grafana/flint/pull/165))
- *(deps)* update dependency npm:markdownlint-cli2 to v0.22.0 ([#154](https://github.com/grafana/flint/pull/154))
- normalize markdownlint-cli2 version banner in e2e snapshots ([#164](https://github.com/grafana/flint/pull/164))
- *(deps)* update dependency npm:@biomejs/biome to v2.4.11 ([#153](https://github.com/grafana/flint/pull/153))
- *(deps)* update dependency npm:prettier to v3.8.2 ([#162](https://github.com/grafana/flint/pull/162))
- *(deps)* update dependency pipx:ruff to v0.15.10 ([#149](https://github.com/grafana/flint/pull/149))
- *(deps)* update rust crate tokio to v1.51.1 ([#155](https://github.com/grafana/flint/pull/155))
- *(deps)* update actions/attest-build-provenance action to v4 ([#159](https://github.com/grafana/flint/pull/159))
- Revert "chore(deps): update dependency github:mvdan/sh to v3.13.1" ([#152](https://github.com/grafana/flint/pull/152))
- *(deps)* update rust crate semver to v1.0.28 ([#150](https://github.com/grafana/flint/pull/150))
- *(deps)* update dependency pipx:codespell to v2.4.2 ([#148](https://github.com/grafana/flint/pull/148))
- *(deps)* update dependency actions/checkout to v6.0.2 ([#147](https://github.com/grafana/flint/pull/147))
- *(deps)* update dependency github:mvdan/sh to v3.13.1 ([#151](https://github.com/grafana/flint/pull/151))
- *(deps)* update dependency grafana/flint to v0.9.2 ([#141](https://github.com/grafana/flint/pull/141))
- *(deps)* update dependency npm:renovate to v43.104.1 ([#144](https://github.com/grafana/flint/pull/144))
- *(deps)* update dependency mise to v2026.4.1 ([#143](https://github.com/grafana/flint/pull/143))
- *(main)* release 0.9.2 ([#133](https://github.com/grafana/flint/pull/133))
- *(deps)* update dependency mise to v2026.3.16 ([#130](https://github.com/grafana/flint/pull/130))
- *(deps)* update dependency npm:renovate to v43.92.1 ([#131](https://github.com/grafana/flint/pull/131))
- *(deps)* update node.js to v24.14.1 ([#129](https://github.com/grafana/flint/pull/129))
- *(deps)* update jdx/mise-action action to v4.0.1 ([#128](https://github.com/grafana/flint/pull/128))
- *(deps)* update dependency mise to v2026.3.9 ([#126](https://github.com/grafana/flint/pull/126))
- *(deps)* update dependency npm:renovate to v43.84.0 ([#127](https://github.com/grafana/flint/pull/127))
- *(deps)* update dependency grafana/flint to v0.9.1 ([#125](https://github.com/grafana/flint/pull/125))
- *(main)* release 0.9.1 ([#124](https://github.com/grafana/flint/pull/124))
- *(main)* release 0.9.0 ([#120](https://github.com/grafana/flint/pull/120))
- throttle flint Renovate updates to once a week ([#113](https://github.com/grafana/flint/pull/113))
- *(deps)* update dependency npm:renovate to v43.73.2 ([#121](https://github.com/grafana/flint/pull/121))
- Reduce renovate update frequency ([#122](https://github.com/grafana/flint/pull/122))
- *(deps)* update dependency npm:renovate to v43.73.1 ([#119](https://github.com/grafana/flint/pull/119))
- *(deps)* update dependency npm:renovate to v43.71.0 ([#118](https://github.com/grafana/flint/pull/118))
- *(deps)* update dependency npm:renovate to v43.70.0 ([#117](https://github.com/grafana/flint/pull/117))
- *(deps)* update ghcr.io/super-linter/super-linter docker tag to slim-v8.5.0 ([#115](https://github.com/grafana/flint/pull/115))
- *(deps)* update dependency npm:renovate to v43.66.5 ([#108](https://github.com/grafana/flint/pull/108))
- *(deps)* update jdx/mise-action action to v4 ([#116](https://github.com/grafana/flint/pull/116))
- add CODEOWNERS file for SDK team ([#114](https://github.com/grafana/flint/pull/114))
- *(deps)* update dependency grafana/flint to v0.8.0 ([#109](https://github.com/grafana/flint/pull/109))
- *(deps)* update dependency mise to v2026.3.8 ([#110](https://github.com/grafana/flint/pull/110))
- *(deps)* update dependency npm:renovate to v43.61.0 ([#105](https://github.com/grafana/flint/pull/105))
- *(deps)* update dependency npm:renovate to v43.59.5 ([#104](https://github.com/grafana/flint/pull/104))
- *(main)* release 0.8.0 ([#96](https://github.com/grafana/flint/pull/96))
- *(deps)* update dependency npm:renovate to v43.59.3 ([#101](https://github.com/grafana/flint/pull/101))
- *(deps)* update dependency npm:renovate to v43.59.2 ([#99](https://github.com/grafana/flint/pull/99))
- *(deps)* update dependency npm:renovate to v43.58.0 ([#88](https://github.com/grafana/flint/pull/88))
- *(deps)* update jdx/mise-action action to v3.6.3 ([#91](https://github.com/grafana/flint/pull/91))
- *(deps)* update dependency grafana/flint to v0.7.1 ([#92](https://github.com/grafana/flint/pull/92))
- *(deps)* update dependency mise to v2026.3.3 ([#94](https://github.com/grafana/flint/pull/94))
- *(main)* release 0.7.1 ([#87](https://github.com/grafana/flint/pull/87))
- add lychee config cleanup note for lint:links adopters ([#61](https://github.com/grafana/flint/pull/61))
- *(deps)* update dependency npm:renovate to v43.45.1 ([#85](https://github.com/grafana/flint/pull/85))
- *(deps)* update dependency npm:renovate to v43.43.2 ([#83](https://github.com/grafana/flint/pull/83))
- *(deps)* update dependency mise to v2026.2.21 ([#84](https://github.com/grafana/flint/pull/84))
- *(deps)* update dependency npm:renovate to v43.42.1 ([#82](https://github.com/grafana/flint/pull/82))
- *(deps)* update dependency npm:renovate to v43.40.2 ([#81](https://github.com/grafana/flint/pull/81))
- *(deps)* update dependency npm:renovate to v43.40.0 ([#80](https://github.com/grafana/flint/pull/80))
- *(deps)* update dependency npm:renovate to v43.39.2 ([#79](https://github.com/grafana/flint/pull/79))
- *(deps)* update dependency npm:renovate to v43.38.1 ([#78](https://github.com/grafana/flint/pull/78))
- *(deps)* update dependency npm:renovate to v43.38.0 ([#77](https://github.com/grafana/flint/pull/77))
- *(deps)* update dependency npm:renovate to v43.36.2 ([#76](https://github.com/grafana/flint/pull/76))
- *(deps)* update dependency npm:renovate to v43.35.1 ([#75](https://github.com/grafana/flint/pull/75))
- *(deps)* update dependency npm:renovate to v43.34.0 ([#74](https://github.com/grafana/flint/pull/74))
- *(deps)* update dependency npm:renovate to v43.33.1 ([#73](https://github.com/grafana/flint/pull/73))
- *(deps)* update dependency npm:renovate to v43.32.2 ([#72](https://github.com/grafana/flint/pull/72))
- *(deps)* update dependency npm:renovate to v43.32.1 ([#71](https://github.com/grafana/flint/pull/71))
- *(deps)* update dependency npm:renovate to v43.31.9 ([#70](https://github.com/grafana/flint/pull/70))
- *(deps)* update dependency npm:renovate to v43.31.8 ([#69](https://github.com/grafana/flint/pull/69))
- *(deps)* update node.js to v24.14.0 ([#68](https://github.com/grafana/flint/pull/68))
- *(deps)* update dependency npm:renovate to v43.31.7 ([#67](https://github.com/grafana/flint/pull/67))
- *(deps)* update dependency grafana/flint to v0.7.0 ([#66](https://github.com/grafana/flint/pull/66))
- *(deps)* update dependency npm:renovate to v43.31.3 ([#65](https://github.com/grafana/flint/pull/65))
- *(deps)* update dependency npm:renovate to v43.31.1 ([#64](https://github.com/grafana/flint/pull/64))
- *(deps)* update dependency npm:renovate to v43.31.0 ([#63](https://github.com/grafana/flint/pull/63))
- *(deps)* update dependency npm:renovate to v43.30.1 ([#62](https://github.com/grafana/flint/pull/62))
- *(deps)* update dependency npm:renovate to v43.29.2 ([#60](https://github.com/grafana/flint/pull/60))
- rename CLAUDE.md to AGENTS.md ([#51](https://github.com/grafana/flint/pull/51))
- *(deps)* update dependency npm:renovate to v43.29.0 ([#59](https://github.com/grafana/flint/pull/59))
- *(main)* release 0.7.0 ([#41](https://github.com/grafana/flint/pull/41))
- *(deps)* update dependency npm:renovate to v43.26.5 ([#57](https://github.com/grafana/flint/pull/57))
- *(deps)* update dependency npm:renovate to v43.26.4 ([#55](https://github.com/grafana/flint/pull/55))
- *(deps)* update dependency npm:renovate to v43.26.2 ([#54](https://github.com/grafana/flint/pull/54))
- *(deps)* update dependency npm:renovate to v43.26.0 ([#53](https://github.com/grafana/flint/pull/53))
- *(deps)* update dependency npm:renovate to v43.25.11 ([#52](https://github.com/grafana/flint/pull/52))
- *(deps)* update dependency npm:renovate to v43.25.8 ([#50](https://github.com/grafana/flint/pull/50))
- *(deps)* update dependency npm:renovate to v43.25.6 ([#49](https://github.com/grafana/flint/pull/49))
- *(deps)* update dependency npm:renovate to v43.25.3 ([#48](https://github.com/grafana/flint/pull/48))
- *(deps)* update dependency npm:renovate to v43.25.2 ([#47](https://github.com/grafana/flint/pull/47))
- *(deps)* update dependency npm:renovate to v43.24.2 ([#46](https://github.com/grafana/flint/pull/46))
- *(deps)* update dependency npm:renovate to v43.24.1 ([#45](https://github.com/grafana/flint/pull/45))
- *(deps)* update dependency npm:renovate to v43.24.0 ([#44](https://github.com/grafana/flint/pull/44))
- Add mise version pinning custom manager ([#43](https://github.com/grafana/flint/pull/43))
- *(release-please)* remove unnecessary separate-pull-requests workaround ([#42](https://github.com/grafana/flint/pull/42))
- *(deps)* update dependency npm:renovate to v43.22.0 ([#39](https://github.com/grafana/flint/pull/39))
- add Renovate preset to Usage section ([#38](https://github.com/grafana/flint/pull/38))
- *(deps)* update dependency npm:renovate to v43.19.2 ([#37](https://github.com/grafana/flint/pull/37))
- *(deps)* update dependency npm:renovate to v43.19.0 ([#36](https://github.com/grafana/flint/pull/36))
- *(deps)* update dependency npm:renovate to v43.18.0 ([#35](https://github.com/grafana/flint/pull/35))
- *(deps)* update dependency npm:renovate to v43.16.0 ([#34](https://github.com/grafana/flint/pull/34))
- *(deps)* update dependency npm:renovate to v43.15.3 ([#33](https://github.com/grafana/flint/pull/33))
- *(deps)* update dependency npm:renovate to v43.15.1 ([#32](https://github.com/grafana/flint/pull/32))
- update README URLs to v0.6.0 and add Renovate rule to keep them current ([#31](https://github.com/grafana/flint/pull/31))
- *(release-please)* add footer to release PRs with CI trigger reminder ([#30](https://github.com/grafana/flint/pull/30))
- *(main)* release 0.6.0 ([#29](https://github.com/grafana/flint/pull/29))
- *(deps)* update dependency npm:renovate to v43.14.1 ([#26](https://github.com/grafana/flint/pull/26))
- *(main)* release 0.5.0 ([#25](https://github.com/grafana/flint/pull/25))
- explain role of mise and Renovate, add editorconfig ([#23](https://github.com/grafana/flint/pull/23))
- *(main)* release 0.4.0 ([#22](https://github.com/grafana/flint/pull/22))
- document SHA-pinned URLs for flint task consumption ([#20](https://github.com/grafana/flint/pull/20))
- *(main)* release 0.3.0 ([#16](https://github.com/grafana/flint/pull/16))
- add release impact guidance to commit message conventions ([#15](https://github.com/grafana/flint/pull/15))
- add example project and remove redundant release-please PR comment ([#14](https://github.com/grafana/flint/pull/14))
- *(main)* release 0.2.0 ([#9](https://github.com/grafana/flint/pull/9))
- improve release-please PR workflow and exclude generated files from linting ([#11](https://github.com/grafana/flint/pull/11))
- replace commitlint with PR title validation ([#8](https://github.com/grafana/flint/pull/8))
- enable conventional commit linting and add changelog commits ([#6](https://github.com/grafana/flint/pull/6))
- Consolidate link lint scripts and add README badges ([#5](https://github.com/grafana/flint/pull/5))
- Add Release Please for automated releases ([#4](https://github.com/grafana/flint/pull/4))
- Address PR #1 review, add Renovate auto-update docs ([#3](https://github.com/grafana/flint/pull/3))
- Address review comments from PR #1 ([#2](https://github.com/grafana/flint/pull/2))
- Initial commit ([#1](https://github.com/grafana/flint/pull/1))
- Initial commit

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
