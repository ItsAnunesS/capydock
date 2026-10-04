import {
  mkdir,
  copyFile,
  chmod,
  writeFile,
  symlink,
  lstat,
  readlink,
  unlink,
  readFile,
} from "node:fs/promises";
import { homedir } from "node:os";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const root = fileURLToPath(new URL("../", import.meta.url));
const local = resolve(homedir(), ".local");
const data = process.env.XDG_DATA_HOME || resolve(local, "share");
// Installation and data identities stay stable when the display name changes.
const installed = resolve(data, "proton-drive-desktop/app");
const managed = resolve(data, "io.github.protondrive.desktop/bin");
const release = JSON.parse(
  await readFile(resolve(root, "bin/release.json"), "utf8"),
);
const cli = await readFile(resolve(root, "bin/proton-drive"));
if (createHash("sha512").update(cli).digest("hex") !== release.sha512)
  throw new Error("The bundled CLI does not match its official checksum.");

const computersRelease = JSON.parse(
  await readFile(resolve(root, "bin/computers-release.json"), "utf8"),
);
if (
  createHash("sha512")
    .update(await readFile(resolve(root, "bin/proton-drive-computers")))
    .digest("hex") !== computersRelease.sha512
)
  throw new Error("The Computers helper does not match its build checksum.");

async function copy(source, destination, executable = false) {
  await mkdir(dirname(destination), { recursive: true });
  // Rename avoids ETXTBSY when upgrading a running copy.
  const temporary = destination + ".installing";
  await copyFile(source, temporary);
  if (executable) await chmod(temporary, 0o755);
  const { rename } = await import("node:fs/promises");
  await rename(temporary, destination);
}
async function link(target, destination, previous) {
  await mkdir(dirname(destination), { recursive: true });
  try {
    const info = await lstat(destination);
    if (!info.isSymbolicLink())
      throw new Error(
        `A file already exists at ${destination}; the installation was preserved.`,
      );
    const current = await readlink(destination);
    if (current !== target && current !== previous)
      throw new Error(
        `The link ${destination} belongs to another installation.`,
      );
    await unlink(destination);
  } catch (e) {
    if (e.code !== "ENOENT") throw e;
  }
  await symlink(target, destination);
}

await copy(
  resolve(root, "target/release/proton-drive-desktop"),
  resolve(installed, "proton-drive-desktop"),
  true,
);
await copy(
  resolve(root, "src-tauri/icons/icon.png"),
  resolve(installed, "icon.png"),
);
await copy(
  resolve(root, "bin/proton-drive"),
  resolve(installed, "bin/proton-drive"),
  true,
);
await copy(
  resolve(root, "bin/release.json"),
  resolve(installed, "bin/release.json"),
);
await copy(
  resolve(root, "bin/PROTON-SDK-LICENSE.md"),
  resolve(installed, "bin/PROTON-SDK-LICENSE.md"),
);
for (const notice of ["LICENSE", "ASSETS_LICENSE.md", "THIRD_PARTY_NOTICES.md"])
  await copy(resolve(root, notice), resolve(installed, notice));
await copy(
  resolve(root, "THIRD_PARTY_NOTICES.md"),
  resolve(installed, "THIRD_PARTY_NOTICES.md"),
);
await copy(
  resolve(root, "bin/proton-drive-computers"),
  resolve(installed, "bin/proton-drive-computers"),
  true,
);
await copy(
  resolve(root, "bin/computers-release.json"),
  resolve(installed, "bin/computers-release.json"),
);
try {
  await lstat(resolve(managed, "proton-drive"));
} catch (e) {
  if (e.code !== "ENOENT") throw e;
  await copy(
    resolve(root, "bin/proton-drive"),
    resolve(managed, "proton-drive"),
    true,
  );
  await copy(
    resolve(root, "bin/release.json"),
    resolve(managed, "release.json"),
  );
}
await link(
  resolve(installed, "proton-drive-desktop"),
  resolve(local, "bin/capydock"),
);
await link(
  resolve(installed, "proton-drive-desktop"),
  resolve(local, "bin/proton-drive-desktop"),
);
await link(
  resolve(managed, "proton-drive"),
  resolve(local, "bin/proton-drive"),
  resolve(root, "bin/proton-drive"),
);
const applications = resolve(data, "applications");
await mkdir(applications, { recursive: true });
const execPath = resolve(installed, "proton-drive-desktop").replace(
  /[\\"`$]/g,
  "\\$&",
);
await writeFile(
  resolve(applications, "io.github.protondrive.desktop.desktop"),
  `[Desktop Entry]\nType=Application\nName=CapyDock\nComment=Sync your folders with Proton Drive\nComment[pt]=Sincronize suas pastas com o Proton Drive\nComment[pt_BR]=Sincronize suas pastas com o Proton Drive\nComment[es]=Sincroniza tus carpetas con Proton Drive\nExec="${execPath}"\nIcon=${resolve(installed, "icon.png")}\nTerminal=false\nCategories=Network;FileTransfer;\nStartupWMClass=proton-drive-desktop\n`,
);
console.log(
  `Application: ${installed}/proton-drive-desktop\nCLI managed by the app: ${managed}/proton-drive\nCommands: capydock and proton-drive (proton-drive-desktop remains available)\nShortcut added to the applications menu.`,
);
