import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { releaseFiles, assertVersion } from "./version.mjs";
import {
  findAppImage,
  verifyAppImage,
  verifyZsync,
  hashFile,
} from "./appimage.mjs";

const plan = JSON.parse(
  await readFile("release-dist/release-plan.json", "utf8"),
);
assertVersion(plan.version);
const image = await findAppImage();
const components = await verifyAppImage(image);
const name = `CapyDock_${plan.version}_x86_64.AppImage`;
await verifyZsync(image, `${image}.zsync`, name);
await mkdir("release-dist/assets", { recursive: true });
await copyFile(image, resolve("release-dist/assets", name));
await copyFile(
  `${image}.zsync`,
  resolve("release-dist/assets", `${name}.zsync`),
);
const provenance = {
  app: "CapyDock",
  version: plan.version,
  sourceCommit: plan.commit,
  platform: "linux/x86_64",
  ...components,
};
await writeFile(
  "release-dist/assets/build-info.json",
  JSON.stringify(provenance, null, 2) + "\n",
);
const assets = [name, `${name}.zsync`, "build-info.json"];
const sums = await Promise.all(
  assets.map(
    async (file) =>
      `${await hashFile(resolve("release-dist/assets", file), "sha256")}  ${file}\n`,
  ),
);
await writeFile("release-dist/assets/SHA256SUMS", sums.join(""));
for (const file of releaseFiles) {
  const destination = resolve("release-dist/source", file);
  await mkdir(dirname(destination), { recursive: true });
  await copyFile(file, destination);
}
console.log(`Prepared ${name}, checksums and build provenance.`);
