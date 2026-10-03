import { createReadStream } from "node:fs";
import {
  chmod,
  copyFile,
  mkdtemp,
  open,
  readFile,
  readdir,
  rename,
  rm,
} from "node:fs/promises";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { homedir, tmpdir } from "node:os";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const components = [
  ["protonCli", "release.json", "proton-drive"],
  ["computersHelper", "computers-release.json", "proton-drive-computers"],
];
const resourcePath = "usr/lib/CapyDock/bin";

export async function hashFile(path, algorithm = "sha512") {
  const hash = createHash(algorithm);
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest("hex");
}

async function sourceComponents(cwd) {
  const result = [];
  for (const [key, metadata, binary] of components) {
    const metadataBytes = await readFile(resolve(cwd, "bin", metadata));
    const value = JSON.parse(metadataBytes);
    if (
      !/^[a-f0-9]{128}$/.test(value.sha512) ||
      value.sha512 !== (await hashFile(resolve(cwd, "bin", binary)))
    )
      throw new Error(`Source executable differs from its metadata: ${binary}`);
    result.push({ key, metadata, binary, metadataBytes, value });
  }
  return result;
}

export async function restoreEmbeddedExecutables(appDir, cwd = process.cwd()) {
  // linuxdeploy rewrites RPATH even on self-contained resource executables.
  // Validate every source first, then restore their exact authenticated bytes.
  const sources = await sourceComponents(cwd);
  for (const { metadata, binary } of sources) {
    for (const file of [metadata, binary])
      await copyFile(
        resolve(cwd, "bin", file),
        resolve(appDir, resourcePath, file),
      );
    await chmod(resolve(appDir, resourcePath, binary), 0o755);
  }
}

export async function verifyEmbeddedExecutables(appDir, cwd = process.cwd()) {
  const provenance = {};
  for (const {
    key,
    metadata,
    binary,
    metadataBytes,
    value,
  } of await sourceComponents(cwd)) {
    if (
      !metadataBytes.equals(
        await readFile(resolve(appDir, resourcePath, metadata)),
      ) ||
      value.sha512 !== (await hashFile(resolve(appDir, resourcePath, binary)))
    )
      throw new Error(
        `Embedded executable differs from its metadata: ${binary}`,
      );
    provenance[key] = value;
  }
  return provenance;
}

export async function findAppImage(cwd = process.cwd()) {
  const bundle = resolve(cwd, "target/release/bundle/appimage");
  const images = (await readdir(bundle)).filter((file) =>
    file.endsWith(".AppImage"),
  );
  if (images.length !== 1)
    throw new Error(`Expected exactly one AppImage, found ${images.length}`);
  return resolve(bundle, images[0]);
}

export async function verifyAppImage(path, cwd = process.cwd()) {
  const file = await open(path, "r");
  const header = Buffer.alloc(11);
  try {
    await file.read(header, 0, header.length, 0);
  } finally {
    await file.close();
  }
  if (
    header.subarray(0, 4).toString("hex") !== "7f454c46" ||
    header.subarray(8, 11).toString("hex") !== "414902"
  )
    throw new Error("The bundle is not a type-2 AppImage.");
  const directory = await mkdtemp(
    resolve(tmpdir(), "capydock-appimage-check-"),
  );
  try {
    // Inspect the actual final image, not just the build directory.
    execFileSync(path, ["--appimage-extract", `${resourcePath}/*`], {
      cwd: directory,
      stdio: ["ignore", "ignore", "pipe"],
      timeout: 120_000,
    });
    return await verifyEmbeddedExecutables(
      resolve(directory, "squashfs-root"),
      cwd,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
}

async function repack() {
  const cwd = process.cwd();
  const image = await findAppImage(cwd);
  const appDir = resolve(cwd, "target/release/bundle/appimage/CapyDock.AppDir");
  const plugin =
    process.env.CAPYDOCK_APPIMAGE_PLUGIN ||
    resolve(
      process.env.XDG_CACHE_HOME || resolve(homedir(), ".cache"),
      "tauri/linuxdeploy-plugin-appimage.AppImage",
    );
  await restoreEmbeddedExecutables(appDir, cwd);
  await verifyEmbeddedExecutables(appDir, cwd);
  const staging = await mkdtemp(
    resolve(cwd, "target/release/bundle/appimage/.repack-"),
  );
  const output = resolve(staging, "CapyDock.AppImage");
  try {
    // The standalone output plugin compresses the AppDir without changing ELF files.
    execFileSync(plugin, [`--appdir=${appDir}`], {
      cwd,
      stdio: "inherit",
      timeout: 600_000,
      env: {
        ...process.env,
        APPIMAGE_EXTRACT_AND_RUN: "1",
        ARCH: "x86_64",
        LDAI_OUTPUT: output,
        LINUXDEPLOY_OUTPUT_VERSION: JSON.parse(
          await readFile(resolve(cwd, "package.json")),
        ).version,
      },
    });
    await verifyAppImage(output, cwd);
    await rename(output, image);
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
  console.log(
    "Repacked AppImage; both embedded executables match their SHA-512 checksums.",
  );
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
)
  await repack();
