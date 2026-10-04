# CapyDock builds and releases

The first public release is **v0.1.0**. The only application package published is the **Linux x86-64 AppImage**. Its `.AppImage.zsync` companion supports Gear Lever updates. `SHA256SUMS` and `build-info.json` record file integrity and build provenance.

## Automatic workflow

1. Develop on `dev` or open a pull request targeting it. Use Conventional Commits in the final history. A squash merge uses the pull request title as its commit message.
2. A push to `dev` checks versions, formatting, frontend types, interface tests, Rust tests, release tests and Clippy, then compiles the native application. It does not calculate a new version or publish a release.
3. Merge tested changes into `release` to publish them. That branch repeats the checks, calculates the next version from commits since the last reachable release tag and builds the AppImage on Ubuntu 22.04. The Proton CLI is pinned in `bin/release.json` and checked with SHA-512. The Computers helper is built from its pinned SDK revision.
4. The publishing job receives the validated artifact. It creates `chore(release): vX.Y.Z [skip ci]` with matching versions and component metadata, then pushes `release` and its tag atomically without force. Publication advances only `release` and the tag.
5. A draft release receives the AppImage, zsync file, checksums and provenance. It becomes public after all uploads succeed. Release notes come from the commits since the previous tag.

Pull requests validate and compile, including those targeting `release`. Only pushes or manual runs on `release` may publish. Both the workflow and the publication script enforce this restriction. The build job has read permission; only the publishing job has `contents: write`. Publication uses the workflow's `GITHUB_TOKEN` and needs no Proton credentials.

To start a manual run, choose **Actions → CapyDock CI and release → Run workflow → release**. Selecting `dev` runs validation and compilation only. A manual run still follows the commit based version rules; it does not force a new version for documentation changes. The workflow must exist on the default branch for the manual button to appear.

## Version rules

| Commits since the previous tag                               | Result starting at 0.1.0                      |
| :----------------------------------------------------------- | :-------------------------------------------- |
| First publication without a previous release tag             | **0.1.0**, regardless of initial commit types |
| `fix:` or `perf:`                                            | 0.1.1                                         |
| `feat:`                                                      | 0.2.0                                         |
| `feat!:` or a `BREAKING CHANGE:` footer                      | 1.0.0                                         |
| Only `docs`, `chore`, `ci`, `test` or compatible refactoring | A validated build without a new release       |

The greatest impact in the commit set determines the version. Tags outside the branch's history are ignored. Versions follow `MAJOR.MINOR.PATCH`; they are not reset after publication. Let the workflow create release tags.

`package.json`, `package-lock.json`, `src-tauri/tauri.conf.json`, both Cargo package manifests and `Cargo.lock` must agree. `npm run version:check` checks this. Proton CLI and Computers helper versions are independent of the app version.

## After publication

Bring the release bot's version commit back into `dev`, preserving the history and tag:

```sh
git switch dev
git pull --ff-only origin dev
git fetch origin --tags
git merge origin/release
git push origin dev
```

This push does not publish another release. For the next publication, merge `dev` into `release` while preserving the latest release tag's history. Avoid squash merges when synchronizing these two branches.

If `release` advances during a build, the older build does not publish; the following run includes the pending changes. Concurrent work on `dev` is preserved. Runs are serialized per branch and an upload is not canceled automatically. A protected branch that rejects the publishing push causes the workflow to fail without rewriting history. The automation does not disable branch protection.

## Failed publication

If an upload fails after the tag is created, choose **Re-run failed jobs** on the same run. Artifacts are retained for 14 days. Publication checks the original source commit and tag before resuming the matching draft.

If the artifacts have expired, rerun all jobs from the original execution to rebuild the same version. Published assets are not overwritten. Retrying an older release does not promote it over a newer one.

## AppImage packaging and updates

The installed AppImage does not require Node.js, Rust or Bun. The app can update its managed Proton CLI independently. Gear Lever manages updates to the AppImage according to the user's preferences.

Since version 0.1.2, the embedded `.upd_info` channel is:

```text
gh-releases-zsync|ItsAnunesS|capydock|latest|CapyDock_*_x86_64.AppImage.zsync
```

Gear Lever reads this channel and the embedded CapyDock icon when importing the package. Older AppImages need to be replaced once with a package containing this metadata.

Repackaging reads the repository from `GITHUB_REPOSITORY`, or from `origin` for a local build, and generates the zsync file using the final asset name. Validation checks the update channel, desktop entry version, real PNG files at `.DirIcon` and `capydock.png`, and the zsync filename, relative URL, size and SHA-1. The zsync file is included in `SHA256SUMS`. Keep published asset names unchanged.

After Tauri gathers libraries, `npm run release:appimage` restores the exact bundled executables and repackages the AppDir. This is necessary because linuxdeploy can rewrite the RPATH of independent ELF resources. The final AppImage is extracted to verify component metadata and SHA-512 checksums before publication. `CAPYDOCK_APPIMAGE_PLUGIN` can override the cached AppImage output plugin path.

The package includes `LICENSE`, `ASSETS_LICENSE.md`, `THIRD_PARTY_NOTICES.md` and the pinned Proton SDK license. Code and artwork have separate terms. Read the notices before redistributing a build.

## References

1. [Tauri AppImage distribution](https://v2.tauri.app/distribute/appimage/)
2. [AppImage update metadata and zsync](https://docs.appimage.org/packaging-guide/optional/updates.html)
3. [GitHub workflow triggers and token permissions](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow)
4. [Conventional Commits analyzer](https://github.com/semantic-release/commit-analyzer)
