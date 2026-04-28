# Alternatives / Comparisons

This page captures the "why not X?" comparisons that would otherwise clutter
the main [why/principles page](why.md).

## Overview

Ratings are relative and intentionally coarse. The sections below explain the
"why" behind each row in more detail.

| Tool / approach           | Speed                 | Setup effort                  | Cross-platform  | Cross-language | Autofix support        | Delta / diff-aware | Predictable and updatable linter versions | Local == CI               |
| ------------------------- | --------------------- | ----------------------------- | --------------- | -------------- | ---------------------- | ------------------ | ----------------------------------------- | ------------------------- |
| flint                     | high                  | low                           | yes             | yes            | yes, where supported   | yes                | yes                                       | yes                       |
| pre-commit                | medium                | medium                        | yes             | yes            | mixed                  | mixed              | mixed                                     | mixed                     |
| Husky                     | medium for Node repos | medium to high outside Node   | yes             | hook-dependent | hook-dependent         | hook-dependent     | hook-dependent                            | mixed                     |
| Spotless / build plugins  | medium                | medium in matching ecosystems | ecosystem-bound | low to medium  | yes, formatter-focused | usually no         | usually yes in that ecosystem             | usually yes in that build |
| MegaLinter / super-linter | low to medium         | medium                        | yes             | yes            | mixed                  | limited / mixed    | mixed                                     | mixed                     |

Use these sections as relative comparisons against flint on a few recurring
dimensions: speed, setup effort, cross-platform support, cross-language scope,
autofix support, delta/diff awareness, predictable and updatable linter
versions, and how closely local behavior matches CI.

## flint

flint is the reference point for the comparisons on this page: a native lint
runner that discovers active tools from the repo, scopes most checks to changed
files, and keeps the local and CI path aligned.

It is also intentionally opinionated about ownership boundaries. Checks stay
separate instead of being fused into one meta-tool, and overlapping file types
have a clear default owner so repos are not forced to keep deciding which
linter or formatter should govern each domain.

| Dimension                                 | Rating               | Why                                                                                                                                                                            |
| ----------------------------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Speed                                     | high                 | flint runs native tools directly, avoids container startup, and scopes most checks to changed files by default.                                                                |
| Setup effort                              | low                  | `flint init` scaffolds the baseline setup, and most repos only need to choose which tools to enable rather than repeatedly deciding how to compose overlapping tools.          |
| Cross-platform                            | yes                  | flint supports Linux, macOS, and Windows.                                                                                                                                      |
| Cross-language                            | yes                  | It orchestrates multiple language-specific tools behind one runner.                                                                                                            |
| Autofix support                           | yes, where supported | `flint run --fix` uses each tool's fixer when one exists and reports what still needs review.                                                                                  |
| Delta / diff-aware                        | yes                  | Changed-file execution is the default model, with baseline expansion only when coverage changes require it.                                                                    |
| Predictable and updatable linter versions | yes                  | Linter versions are pinned by the repo, so behavior stays stable until the repo intentionally updates to a newer version, for example through Renovate updates to `mise.toml`. |
| Local == CI                               | yes                  | The same binary, config model, and pinned tools are used in both environments.                                                                                                 |

## pre-commit

pre-commit adds a parallel tool management system on top of mise. Consuming
repos already declare their tools in `mise.toml`, so pre-commit means
maintaining a second inventory in `.pre-commit-config.yaml`, with its own
versioning and install lifecycle.

For repos that are already mise-first, that is extra setup and drift surface
without much benefit.

It can also push ownership decisions back onto each repo. Teams still need to
decide which hooks to compose for overlapping domains, and that composition
lives in hook wiring rather than in a single built-in policy.

| Dimension                                 | Rating | Why                                                                                                                                                                                                     |
| ----------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Speed                                     | medium | Hook startup is usually acceptable, but the extra hook layer and environment management add overhead compared with running native tools directly.                                                       |
| Setup effort                              | medium | You need to add and maintain `.pre-commit-config.yaml` and its hook definitions in addition to the repo's normal tool setup, including repo-level decisions about how overlapping hooks should compose. |
| Cross-platform                            | yes    | pre-commit itself is cross-platform.                                                                                                                                                                    |
| Cross-language                            | yes    | It supports many languages through its hook ecosystem.                                                                                                                                                  |
| Autofix support                           | mixed  | Some hooks fix in place, some only report, and behavior depends on the chosen hooks.                                                                                                                    |
| Delta / diff-aware                        | mixed  | Hook-based runs are often scoped to staged files, but broader CI parity and baseline behavior depend on how each hook is configured.                                                                    |
| Predictable and updatable linter versions | mixed  | Hook revisions can be pinned, but version management lives in separate hook configuration instead of flowing through Renovate updates to `mise.toml`.                                                   |
| Local == CI                               | mixed  | Teams often use pre-commit locally but a different command or environment in CI.                                                                                                                        |

## Husky

Husky manages git hooks for Node.js projects and requires `npm install` to
activate. Repos that are not Node-first still need a `package.json` and a dev
dependency just to run hooks.

`flint hook install` writes a single shell script directly to `.git/hooks/`
with no install step and no language runtime dependency.

| Dimension                                 | Rating                                   | Why                                                                                                                                                                       |
| ----------------------------------------- | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Speed                                     | medium in Node repos, lower outside them | The hook runner itself is lightweight, but it still adds a Node-oriented wrapper around the real lint commands, and that wrapper is a worse fit outside Node-first repos. |
| Setup effort                              | low in Node repos, high outside them     | It fits naturally in Node projects, but non-Node repos need extra package-management setup just to get hooks.                                                             |
| Cross-platform                            | yes                                      | Husky works across platforms through Git hooks and Node tooling.                                                                                                          |
| Cross-language                            | hook-dependent                           | Husky can launch anything, but it does not provide language coverage by itself.                                                                                           |
| Autofix support                           | hook-dependent                           | Whether fixes are available depends entirely on the commands wired into the hooks.                                                                                        |
| Delta / diff-aware                        | hook-dependent                           | It can run on changed or staged files, but only if the hook commands are written that way.                                                                                |
| Predictable and updatable linter versions | hook-dependent                           | Husky only runs whatever commands the repo wires into hooks, so version stability depends on those underlying tools and how the repo manages them.                        |
| Local == CI                               | mixed                                    | Husky is usually local-hook infrastructure, while CI often uses separate scripts or commands.                                                                             |

## Spotless and formatter plugins

Spotless runs `google-java-format` as a Maven build phase, which means format
failures block compilation and test runs. flint keeps formatting as a separate
lint step, scoped to changed files, which is a better fit for fast feedback.

That separation matters beyond speed: formatting remains one explicit check
instead of being entangled with compile or test phases, so repos can reason
about ownership and failures more cleanly.

To migrate: remove `spotless-maven-plugin` from `pom.xml` (and any
`spotless.skip` properties), add `"github:google/google-java-format"` to
`[tools]` in `mise.toml`, and run `flint run --fix` once to confirm the repo is
clean.

| Dimension                                 | Rating                        | Why                                                                                                                                                                       |
| ----------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Speed                                     | medium                        | Build-plugin integration is convenient inside the matching ecosystem, but it is not as cheap as directly invoking a small lint runner on changed files.                   |
| Setup effort                              | medium in matching ecosystems | Setup is reasonable when the repo already uses Maven, Gradle, or a similar build tool, but much less general outside that stack.                                          |
| Cross-platform                            | ecosystem-bound               | It follows the portability of the underlying build tool rather than acting as a general cross-platform lint runner.                                                       |
| Cross-language                            | low to medium                 | It is strong within specific ecosystems, but not a general multi-language lint orchestration layer.                                                                       |
| Autofix support                           | yes, formatter-focused        | Formatter plugins are usually good at in-place fixes.                                                                                                                     |
| Delta / diff-aware                        | usually no                    | They commonly run at project or module scope rather than being natively optimized around changed-file diffs.                                                              |
| Predictable and updatable linter versions | usually yes in that ecosystem | Build plugins and formatter versions are often pinned through the build system, but the model is tied to that ecosystem rather than being a general lint-runner property. |
| Local == CI                               | usually yes in that build     | Reusing the same build plugin in local and CI is straightforward when the repo already standardizes on that build system.                                                 |

## MegaLinter and super-linter

Container-based linters such as super-linter and MegaLinter ship their own tool
versions, independent of what the repo pins in `mise.toml`. That breaks the
"declare once, use everywhere" model. Container startup also adds latency to
every run.

They also tend to bundle many checks behind one larger wrapper. That can be
convenient, but it is a worse fit when repos want cleanly separated checks and
explicit style ownership instead of a broad kitchen-sink layer.

| Dimension                                 | Rating          | Why                                                                                                                                                   |
| ----------------------------------------- | --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Speed                                     | low to medium   | Container startup and larger orchestration layers add noticeable latency compared with native direct execution.                                       |
| Setup effort                              | medium          | Centralized meta-linter setup can be convenient, but it introduces its own config model and container-oriented workflow.                              |
| Cross-platform                            | yes             | Container-based runners generally work across platforms where the container runtime is available.                                                     |
| Cross-language                            | yes             | Broad language coverage is one of the main strengths of these tools.                                                                                  |
| Autofix support                           | mixed           | Some integrated tools can fix in place, but support varies across the bundled linter set and may be awkward in container workflows.                   |
| Delta / diff-aware                        | limited / mixed | Some support changed-file or PR-oriented modes, but the model is usually broader and less native than a runner built around git diffs.                |
| Predictable and updatable linter versions | mixed           | The wrapper itself is versioned predictably, but the bundled linter set and containerized execution model can still make upgrades feel more indirect. |
| Local == CI                               | mixed           | CI often uses the canonical containerized flow, while local usage may be slower, less common, or configured differently.                              |
