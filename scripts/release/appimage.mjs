import { createReadStream } from "node:fs";
import {
  chmod,
  copyFile,
  lstat,
  mkdir,
  mkdtemp,
  open,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { homedir, tmpdir } from "node:os";
import { basename, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { assertVersion } from "./version.mjs";

const components = [
  ["protonCli", "release.json", "proton-drive"],
  ["computersHelper", "computers-release.json", "proton-drive-computers"],
];
const resourcePath = "usr/lib/CapyDock/bin";

export function updateInformation(repository) {
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository || ""))
    throw new Error("Expected a GitHub owner/repository for AppImage updates.");
  return `gh-releases-zsync|${repository.replace("/", "|")}|latest|CapyDock_*_x86_64.AppImage.zsync`;
}

function releaseRepository(cwd) {
  if (process.env.GITHUB_REPOSITORY) return process.env.GITHUB_REPOSITORY;
  const remote = execFileSync("git", ["remote", "get-url", "origin"], {
    cwd,
    encoding: "utf8",
  }).trim();
  const match = remote.match(
    /^(?:git@github\.com:|https:\/\/github\.com\/)([^/]+\/[^/]+?)(?:\.git)?$/,
  );
  if (!match) throw new Error("The AppImage update source must be on GitHub.");
  return match[1];
}

async function bundleVersion(cwd) {
  const { version } = JSON.parse(await readFile(resolve(cwd, "package.json")));
  assertVersion(version);
  return version;
}

async function desktopFile(appDir) {
  const files = (await readdir(appDir)).filter((file) =>
    file.endsWith(".desktop"),
  );
  if (files.length !== 1)
    throw new Error("Expected one AppImage desktop entry.");
  return resolve(appDir, files[0]);
}

function desktopEntry(contents) {
  const entry = contents.match(
    /^\[Desktop Entry\]\r?\n([\s\S]*?)(?=^\[|$(?![\s\S]))/m,
  );
  if (!entry) throw new Error("Missing AppImage Desktop Entry group.");
  return entry;
}

export async function prepareDesktopMetadata(appDir, cwd, version) {
  assertVersion(version);
  const path = await desktopFile(appDir);
  let desktop = await readFile(path, "utf8");
  const entry = desktopEntry(desktop);
  const fields = {
    Name: "CapyDock",
    Icon: "capydock",
    "X-AppImage-Name": "CapyDock",
    "X-AppImage-Version": version,
    "X-AppImage-Arch": "x86_64",
  };
  const lines = entry[1]
    .split(/\r?\n/)
    .filter((line) => !Object.hasOwn(fields, line.split("=", 1)[0]));
  desktop = desktop.replace(
    entry[0],
    `[Desktop Entry]\n${lines.filter(Boolean).join("\n")}\n${Object.entries(
      fields,
    )
      .map(([key, value]) => `${key}=${value}`)
      .join("\n")}\n`,
  );
  await writeFile(path, desktop);
  // Some safe extractors expose symlinks as text, leaving Gear Lever without an icon.
  // Keep the root icon and .DirIcon as actual PNG files, independent of symlinks.
  for (const name of [".DirIcon", "capydock.png"]) {
    const destination = resolve(appDir, name);
    await rm(destination, { force: true });
    await copyFile(resolve(cwd, "src-tauri/icons/icon.png"), destination);
  }
  for (const [size, source] of [
    [256, "icon.png"],
    [128, "128x128.png"],
    [32, "32x32.png"],
  ]) {
    const directory = resolve(
      appDir,
      `usr/share/icons/hicolor/${size}x${size}/apps`,
    );
    await mkdir(directory, { recursive: true });
    await copyFile(
      resolve(cwd, "src-tauri/icons", source),
      resolve(directory, "capydock.png"),
    );
  }
}

export async function verifyDesktopMetadata(appDir, cwd, version) {
  const entry = desktopEntry(
    await readFile(await desktopFile(appDir), "utf8"),
  )[1];
  for (const line of [
    "Name=CapyDock",
    "Icon=capydock",
    "X-AppImage-Name=CapyDock",
    `X-AppImage-Version=${version}`,
    "X-AppImage-Arch=x86_64",
  ]) {
    if (!entry.split(/\r?\n/).includes(line))
      throw new Error(`Missing AppImage metadata: ${line}`);
  }
  const expected = await hashFile(resolve(cwd, "src-tauri/icons/icon.png"));
  for (const name of [
    ".DirIcon",
    "capydock.png",
    "usr/share/icons/hicolor/256x256/apps/capydock.png",
  ]) {
    const path = resolve(appDir, name);
    if (!(await lstat(path)).isFile() || (await hashFile(path)) !== expected)
      throw new Error(`AppImage icon must contain the CapyDock PNG: ${name}`);
  }
}

export async function verifyZsync(image, path, expectedName = basename(image)) {
  const bytes = await readFile(path);
  const separator = bytes.indexOf("\n\n");
  if (separator < 0 || separator > 16_384 || separator + 2 === bytes.length)
    throw new Error("Invalid zsync control file.");
  const header = bytes.subarray(0, separator).toString("utf8");
  for (const line of [
    `Filename: ${expectedName}`,
    `URL: ${expectedName}`,
    `Length: ${(await stat(image)).size}`,
    `SHA-1: ${await hashFile(image, "sha1")}`,
  ]) {
    if (!header.split("\n").includes(line))
      throw new Error(`Invalid zsync metadata: ${line}`);
  }
  if (!/^zsync: \d+\.\d+/m.test(header))
    throw new Error("Missing zsync format version.");
}

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
  const info = execFileSync(path, ["--appimage-updateinformation"], {
    encoding: "utf8",
    timeout: 10_000,
  }).trim();
  if (info !== updateInformation(releaseRepository(cwd)))
    throw new Error(
      "Missing or incorrect embedded AppImage update information.",
    );
  const directory = await mkdtemp(
    resolve(tmpdir(), "capydock-appimage-check-"),
  );
  try {
    // Inspect the actual final image, not just the build directory.
    // The type-2 runtime accepts only one extraction pattern per invocation.
    for (const pattern of [
      `${resourcePath}/*`,
      "*.desktop",
      ".DirIcon",
      "capydock.png",
      "usr/share/icons/hicolor/*/apps/capydock.png",
    ])
      execFileSync(path, ["--appimage-extract", pattern], {
        cwd: directory,
        stdio: ["ignore", "ignore", "pipe"],
        timeout: 120_000,
      });
    await verifyDesktopMetadata(
      resolve(directory, "squashfs-root"),
      cwd,
      await bundleVersion(cwd),
    );
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
  const version = await bundleVersion(cwd);
  const info = updateInformation(releaseRepository(cwd));
  const plugin =
    process.env.CAPYDOCK_APPIMAGE_PLUGIN ||
    resolve(
      process.env.XDG_CACHE_HOME || resolve(homedir(), ".cache"),
      "tauri/linuxdeploy-plugin-appimage.AppImage",
    );
  await restoreEmbeddedExecutables(appDir, cwd);
  await verifyEmbeddedExecutables(appDir, cwd);
  await prepareDesktopMetadata(appDir, cwd, version);
  const staging = await mkdtemp(
    resolve(cwd, "target/release/bundle/appimage/.repack-"),
  );
  const output = resolve(staging, `CapyDock_${version}_x86_64.AppImage`);
  try {
    // The standalone output plugin compresses the AppDir without changing ELF files.
    execFileSync(plugin, [`--appdir=${appDir}`], {
      // appimagetool writes the zsync file into its working directory.
      cwd: staging,
      stdio: "inherit",
      timeout: 600_000,
      env: {
        ...process.env,
        APPIMAGE_EXTRACT_AND_RUN: "1",
        ARCH: "x86_64",
        LDAI_OUTPUT: output,
        LDAI_UPDATE_INFORMATION: info,
        LINUXDEPLOY_OUTPUT_VERSION: version,
      },
    });
    await verifyAppImage(output, cwd);
    await verifyZsync(output, `${output}.zsync`);
    await rename(`${output}.zsync`, `${image}.zsync`);
    await rename(output, image);
  } finally {
    await rm(staging, { recursive: true, force: true });
  }
  console.log(
    "Verified AppImage: embedded executables, CapyDock icon, GitHub updates and zsync checksums.",
  );
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
)
  await repack();
