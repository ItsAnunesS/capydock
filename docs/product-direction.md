# CapyDock identity and product direction

CapyDock was chosen on October 3, 2026. The name and capybara identity are used by the app, tray and Linux launcher. Support for additional services remains a product plan.

## Brand

CapyDock suggests a meeting point for files, accounts and devices. Its tagline is **Your files. In good company.**

The logo is a capybara in profile with a broad muzzle, small ear and simple silhouette. Deep green and a soft mineral background provide an identity independent of any cloud provider. Use the original SVG to generate image sizes rather than redrawing copies. Small icons should preserve the animal's silhouette without decorative details.

Original artwork has separate terms in [ASSETS_LICENSE.md](../ASSETS_LICENSE.md). The project's source code and documentation are MIT licensed.

## Installation continuity

The display brand is CapyDock. The existing `io.github.protondrive.desktop` identifier, internal executable, data and recovery paths, and legacy autostart identity remain stable so existing installations keep their credentials, preferences, computer registration and sync pairings. The `capydock` command points to the same installation.

## Current behavior

| Concern                                         | Implemented behavior                                                                                   |
| :---------------------------------------------- | :----------------------------------------------------------------------------------------------------- |
| Registering the same PC under another name      | Reuses the provider UID bound to the account on this installation.                                     |
| Uncertain result after creating a computer      | Saves registration intent first and reuses the original name until its UID is confirmed.               |
| A registered computer still offers registration | Shows its state and actions to view or add folders.                                                    |
| An older registration has no local binding      | Offers an explicit choice of an existing computer.                                                     |
| A remote computer is renamed                    | Identifies it by UID and displays the provider's current name and path.                                |
| A registration disappears from the cloud        | Reports unavailability and preserves existing records.                                                 |
| Library synchronization scope                   | Explains Files, Photos and Albums, with separate treatment for Computers and online documents.         |
| A folder is already paired                      | Opens the existing synchronization management.                                                         |
| Keyboard and visual access                      | Uses visible focus, a skip link, navigation focus, accessible filters, breadcrumbs and table headings. |

Existing duplicate computer records are not merged or removed automatically. Their contents may differ, so cleanup requires an informed choice by the user.

## Multiple services and accounts

The future model should separate these concepts:

| Concept         | Meaning                                                                                                                                      |
| :-------------- | :------------------------------------------------------------------------------------------------------------------------------------------- |
| Provider        | An implementation such as Proton Drive, Google Drive, Dropbox or Syncthing, including its capabilities.                                      |
| Connection      | One account or instance with its own ID, editable name and isolated authentication. Two accounts from one provider are separate connections. |
| Device          | This computer or another known device. Local identity is stable; remote device IDs belong to individual connections.                         |
| File location   | A local root, cloud folder or folder shared through a Syncthing instance.                                                                    |
| Synchronization | A relationship between locations, with direction, conflict and deletion policy, state and queued work.                                       |

Interpret each remote path together with its connection ID. Syncthing needs an adapter for instances, devices and shared folders rather than a forced cloud login or photo library model. Show only the capabilities a provider supports.

## Proposed navigation

1. **Overview:** connection health, active syncs and problems needing attention.
2. **Connections:** connect, authenticate, name and disconnect each account or instance. Show both provider and connection name where relevant.
3. **Files:** choose a connection and browse its supported files, photos, documents or albums.
4. **Synchronizations:** show local folders, connection identified destinations, direction and deletion behavior.
5. **Activity:** show running work, waiting operations and history, with filters by connection.
6. **Settings:** language, tray, startup and app updates. Provider specific options belong to their connection.

Add provider controls when their adapters have been implemented and validated.

## Implementation sequence

1. Introduce connection IDs, display names and provider IDs. Migrate the existing Proton account while preserving pairings, computer UIDs, cache, snapshots and deletion policies.
2. Isolate sessions, credentials, cache, CLI configuration and concurrency limits by connection. The current app uses one CLI session; a list of accounts in the interface would not provide isolation.
3. Associate a connection ID with pairings, tasks, previews, catalog entries and computer bindings. Paths or email addresses alone must not determine identity.
4. Extract the provider contract from real operations and capabilities while retaining the shared scheduler and file protections.
5. Implement one complete additional provider at a time. Validate two accounts from the same service, reconnection, network loss, rate limits and concurrent work before announcing support.
6. Integrate Syncthing using its own capabilities and states, including local network operation and unavailable peers.

## Acceptance criteria

Repeated registration or a changed computer name must reuse the existing binding. A failed response must preserve registration intent. Accounts must never receive another account's actions, cached data or credentials.

Removing a connection must stop or safely schedule its work and explain what happens to local data. File deletion must not be an undisclosed side effect.

Controls must work with a keyboard, icons must have accessible names or be decorative, and errors must have a text explanation. Validate the real desktop app with a screen reader and at 200% zoom; automated tests and browser previews cover only part of this behavior.
