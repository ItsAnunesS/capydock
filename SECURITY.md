# Security policy

CapyDock handles cloud sessions, local files and synchronization decisions. Please report suspected vulnerabilities privately whenever possible.

## Supported versions

Security fixes target the latest stable release on the `release` branch. Older releases are not maintained separately. The `dev` branch is a development branch and is not a supported release channel.

Check [the latest release](https://github.com/ItsAnunesS/capydock/releases/latest) before reporting. The Proton CLI has a separate version and update mechanism; include both the app version and the CLI version when relevant.

## Reporting a vulnerability

Open the repository's [Security advisories page](https://github.com/ItsAnunesS/capydock/security/advisories). If GitHub offers **Report a vulnerability**, use that private reporting form.

If the private form is unavailable, open an [issue](https://github.com/ItsAnunesS/capydock/issues/new) titled **Private security contact request**. Include only a request for a private contact channel. Wait for a maintainer to provide one before sharing technical details. Do not put the vulnerability, exploit, logs or account information in that public issue.

A private report should include:

1. The affected CapyDock and CLI versions, Linux distribution and desktop environment.
2. A description of the issue and its possible impact.
3. Reproduction steps using a test account and disposable files.
4. A minimal example or sanitized evidence, if available.
5. Any suggested fix or mitigation.

Never send passwords, recovery phrases, session tokens, encryption keys, private documents or unredacted application data. Share only the information needed to reproduce the problem.

## Scope and handling

Relevant issues include authentication or account isolation failures, unsafe command execution, path traversal, unintended file access or deletion, exposure of private data and weaknesses in the update or packaging process.

A maintainer will review the report, request clarification when needed and coordinate a fix and disclosure with the reporter. This project does not promise a fixed response time, a bounty or an independent security audit.

Please test only accounts, files and systems you own or are authorized to assess. Avoid destructive tests against a real library. Vulnerabilities in Proton's service should also be reported through [Proton's security channels](https://proton.me/security).

## Local data and updates

Synced files and local previews are decrypted on the computer. Application settings and cached metadata can contain account identifiers, filenames and local paths. Treat exported diagnostics and the application data directory as private.

The bundled CLI is checked against its recorded SHA-512 checksum. AppImage releases include SHA-256 checksums and a Gear Lever update channel. Checksums detect mismatched files; they are not a digital signature or an independent authenticity guarantee. Download releases from the official repository.
