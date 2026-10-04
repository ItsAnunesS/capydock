import { execFileSync } from "node:child_process";
import {
  mkdir,
  copyFile,
  readFile,
  writeFile,
  chmod,
  symlink,
} from "node:fs/promises";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";

const root = fileURLToPath(new URL("../", import.meta.url));
const commit = "5491f2eea473acaaa86b5969774b84610a37bd46"; // official cli/v0.8.0
const source = resolve(
  process.env.PROTON_SDK_SOURCE || resolve(root, "target/computers-sdk"),
);
const bun = process.env.BUN_BINARY || "bun";
const run = (binary, args, cwd = source, env = process.env) =>
  execFileSync(binary, args, { cwd, env, stdio: "inherit" });
if (!existsSync(resolve(source, ".git"))) {
  await mkdir(source, { recursive: true });
  run("git", ["init"]);
  run("git", [
    "remote",
    "add",
    "origin",
    "https://github.com/ProtonDriveApps/sdk.git",
  ]);
  run("git", ["fetch", "--depth", "1", "origin", commit]);
  run("git", ["checkout", "--detach", "FETCH_HEAD"]);
}
const actual = execFileSync("git", ["rev-parse", "HEAD"], {
  cwd: source,
  encoding: "utf8",
}).trim();
if (actual !== commit)
  throw new Error("The SDK must use the pinned commit: " + commit);
run(bun, ["install", "--frozen-lockfile"], resolve(source, "cli"));
// Resolve dependencies of the linked SDK/account sources from the same locked install.
if (!existsSync(resolve(source, "node_modules")))
  await symlink(
    resolve(source, "cli/node_modules"),
    resolve(source, "node_modules"),
  );
for (const name of [
  "proton-drive-computers.ts",
  "ensure-device.mjs",
  "read-folder-batch.mjs",
])
  await copyFile(
    resolve(root, "scripts/computers", name),
    resolve(source, "cli/src", name),
  );
run(
  bun,
  [
    "scripts/build-cli.mjs",
    "src/proton-drive-computers.ts",
    "bun-linux-x64-baseline",
  ],
  resolve(source, "cli"),
  {
    ...process.env,
    CLI_APP_VERSION_NAME: "external-drive-desktop",
    CLI_VERSION: "0.6.0",
    JS_VERSION: "0.0.0+5491f2e",
    SENTRY_DSN: "",
  },
);
const binary = resolve(root, "bin/proton-drive-computers");
await copyFile(
  resolve(source, "cli/release/linux-x64-baseline/proton-drive-computers"),
  binary,
);
await chmod(binary, 0o755);
await writeFile(
  resolve(root, "bin/computers-release.json"),
  JSON.stringify(
    {
      version: "0.6.0",
      source: "https://github.com/ProtonDriveApps/sdk",
      commit,
      sha512: createHash("sha512")
        .update(await readFile(binary))
        .digest("hex"),
    },
    null,
    2,
  ) + "\n",
);
console.log("Computers helper built: " + binary);
