# Contributing to CapyDock

CapyDock currently supports one Proton Drive account. Work toward additional providers and independent connections is described in [the product direction](docs/product-direction.md). Discuss substantial changes in an issue before building them so the behavior and scope are clear.

## Getting started

Use the prerequisites and setup commands in the [README](README.md#build-from-source). Create your working branch from `dev` and target `dev` when opening a normal pull request. The `release` branch is used to publish tested changes.

Use a test account and disposable folders when working on synchronization. The automated fixtures do not require access to a real Proton account. Do not commit credentials, application data, local logs or downloaded binaries.

## Language and interface

Write source code, identifiers, comments, commit messages, documentation and default messages in English. English is the default and fallback locale. User facing messages belong in the English, Portuguese and Spanish catalogs together.

Use English message keys. `app/i18n/legacy-keys.json` exists only to read activity saved by earlier releases; do not add new Portuguese source keys. Keep filenames, folder names and other user data separate from translatable messages. Translations and Unicode test fixtures may use their intended languages.

Reuse existing components and styles. Keep keyboard navigation, visible focus, accessible labels and reduced motion working. Explain errors with text and a recovery action where possible.

## Changes that affect files

Preserve existing files, synchronization pairings, computer registrations and account boundaries. Changes to deletion, conflict handling, recovery, path validation or background work need tests for the relevant failure cases as well as the successful operation.

Document any migration or changed behavior. Do not silently reset user preferences, create duplicate computer registrations or replace a folder on an unverified disk.

## Validation

Run the checks relevant to your change. The release workflow runs the complete set:

```sh
npm run version:check
npm run format:check
npm run typecheck
npm run test:ui
npm run test:release
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
```

Frontend changes should also be checked in a browser preview. Desktop behavior such as tray actions, native dialogs and closing the window needs a native check when affected. A preview cannot verify synchronization or native integration.

Documentation changes should have working links, commands that match the scripts and a clear distinction between available features and future plans.

## Commits and pull requests

Use Conventional Commits, such as `fix(sync): preserve a file after reconnecting` or `docs: clarify AppImage installation`. Explain the user visible result, relevant validation and remaining limitations in the pull request. Keep unrelated changes in separate commits.

Version numbers and release tags are managed by the workflow. Do not bump one manifest in isolation or create release tags manually. Read [the release guide](docs/releases.md) before changing packaging or publishing.

Report security concerns through [SECURITY.md](SECURITY.md), not through a public bug report. Keep discussions respectful, focus feedback on the work and avoid sharing other people's private information.

## Licensing contributions

Submit original code and documentation under the MIT license in [LICENSE](LICENSE), or identify compatible third party material and preserve its notices. Do not submit work you lack permission to contribute.

Artwork is governed separately by [ASSETS_LICENSE.md](ASSETS_LICENSE.md). Discuss artwork contributions and the rights needed to distribute them before submitting them. The code license does not grant permission to reuse the CapyDock logo or other restricted art in a distributed fork.
