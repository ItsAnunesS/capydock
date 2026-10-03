import {
  copyFile,
  mkdir,
  readFile,
  readdir,
  writeFile,
} from "node:fs/promises";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { releaseFiles, assertVersion } from "./version.mjs";

const plan = JSON.parse(
  await readFile("release-dist/release-plan.json", "utf8"),
);
assertVersion(plan.version);
const bundle = "target/release/bundle/appimage";
const images = (await readdir(bundle)).filter((file) =>
  file.endsWith(".AppImage"),
);
if (images.length !== 1)
  throw new Error(`Expected exactly one AppImage, found ${images.length}`);
const bytes = await readFile(resolve(bundle, images[0]));
if (
  bytes.subarray(0, 4).toString("hex") !== "7f454c46" ||
  bytes.subarray(8, 11).toString("hex") !== "414902"
)
  throw new Error("The bundle is not a type-2 AppImage.");
const name = `CapyDock_${plan.version}_x86_64.AppImage`;
await mkdir("release-dist/assets", { recursive: true });
await copyFile(
  resolve(bundle, images[0]),
  resolve("release-dist/assets", name),
);
const provenance = {
  app: "CapyDock",
  version: plan.version,
  sourceCommit: plan.commit,
  platform: "linux/x86_64",
};
for (const [key, metadata, binary] of [
  ["protonCli", "bin/release.json", "bin/proton-drive"],
  [
    "computersHelper",
    "bin/computers-release.json",
    "bin/proton-drive-computers",
  ],
]) {
  const value = JSON.parse(await readFile(metadata, "utf8"));
  if (
    value.sha512 !==
    createHash("sha512")
      .update(await readFile(binary))
      .digest("hex")
  )
    throw new Error(`Bundled executable differs from its metadata: ${binary}`);
  provenance[key] = value;
}
await writeFile(
  "release-dist/assets/build-info.json",
  JSON.stringify(provenance, null, 2) + "\n",
);
const assets = [name, "build-info.json"];
const sums = await Promise.all(
  assets.map(
    async (file) =>
      `${createHash("sha256")
        .update(await readFile(resolve("release-dist/assets", file)))
        .digest("hex")}  ${file}\n`,
  ),
);
await writeFile("release-dist/assets/SHA256SUMS", sums.join(""));
for (const file of releaseFiles) {
  const destination = resolve("release-dist/source", file);
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(file, destination);
}
console.log(`Prepared ${name}, checksums and build provenance.`);
