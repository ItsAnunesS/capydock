# Third party notices

CapyDock is an independent project. Proton and Proton Drive are trademarks of their respective owners. Their inclusion does not imply affiliation, sponsorship or endorsement.

The project's original code is covered by [LICENSE](LICENSE). Original CapyDock artwork is covered separately by [ASSETS_LICENSE.md](ASSETS_LICENSE.md). Neither document replaces a third party component's license or the terms of the Proton Drive service.

## Official Proton Drive CLI

The bundled `proton-drive` executable is an official Proton binary distributed without modification. Its version, HTTPS download source, platform and SHA-512 checksum are recorded in `bin/release.json`. CapyDock does not claim authorship of this executable.

The [official Proton Drive SDK repository](https://github.com/ProtonDriveApps/sdk) contains the CLI and SDK sources. The license for the pinned SDK source is preserved in [bin/PROTON-SDK-LICENSE.md](bin/PROTON-SDK-LICENSE.md), including Proton AG's copyright notice. Dependencies and the CLI runtime retain their own license terms.

## Computers helper

The `proton-drive-computers` executable is built by CapyDock from pinned official CLI and SDK sources. It adds computer registration and batched metadata commands. It identifies itself as `external-drive-desktop` and is not an official Proton binary.

| Item                           | Source                                                                                                                           |
| :----------------------------- | :------------------------------------------------------------------------------------------------------------------------------- |
| Base SDK revision              | [5491f2eea473acaaa86b5969774b84610a37bd46](https://github.com/ProtonDriveApps/sdk/tree/5491f2eea473acaaa86b5969774b84610a37bd46) |
| Project adaptations            | `scripts/computers/`                                                                                                             |
| Build procedure                | `scripts/build-computers.mjs`                                                                                                    |
| Version, revision and checksum | `bin/computers-release.json`                                                                                                     |
| SDK license                    | `bin/PROTON-SDK-LICENSE.md`                                                                                                      |
| Embedded Bun runtime           | [Bun 1.3.14](https://github.com/oven-sh/bun/tree/bun-v1.3.14), MIT licensed, with its own dependency notices                     |

The original SDK and runtime notices continue to apply to the helper. CapyDock's MIT license covers only its original adaptations and does not replace upstream notices.

## Interface and desktop dependencies

Tauri, Nuxt, Vue, DaisyUI, Tailwind CSS, Lucide and all other dependencies retain their respective licenses and copyright notices. `package-lock.json` and `Cargo.lock` identify the dependency versions used by a build. Consult the license files and notices distributed with each package for its complete terms.

PDF.js (`pdfjs-dist`) is developed by the Mozilla Foundation and contributors and is licensed under Apache-2.0. Its source and notices are available in the [PDF.js repository](https://github.com/mozilla/pdf.js).

Lucide interface icons are third party assets. The restriction on original CapyDock artwork does not apply to them.

## Release provenance

Every published AppImage has a companion `build-info.json` recording the CapyDock version, source commit and embedded component metadata. `SHA256SUMS` verifies the release files. These records describe the build; they do not grant additional rights over third party components.

When redistributing a permitted build, retain all applicable licenses and copyright notices and follow the separate artwork terms. Review the licenses of any dependency you add or replace.
