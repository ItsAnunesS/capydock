import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const versionFiles = [
  "package.json",
  "package-lock.json",
  "src-tauri/tauri.conf.json",
  "src-tauri/Cargo.toml",
  "crates/drive-core/Cargo.toml",
  "Cargo.lock",
];
export const releaseFiles = [
  ...versionFiles,
  "bin/release.json",
  "bin/computers-release.json",
];
export function assertVersion(version) {
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(version))
    throw new Error(`Expected a stable MAJOR.MINOR.PATCH version: ${version}`);
}

export async function setVersion(version, cwd = process.cwd(), check = false) {
  assertVersion(version);
  const updates = [];
  for (const file of versionFiles) {
    const before = await readFile(resolve(cwd, file), "utf8");
    let after;
    if (file.endsWith(".json")) {
      const value = JSON.parse(before);
      if (check && value.version !== version)
        throw new Error(`Version differs in ${file}`);
      value.version = version;
      if (file === "package-lock.json") {
        if (check && value.packages[""].version !== version)
          throw new Error(`Root package version differs in ${file}`);
        value.packages[""].version = version;
      }
      after = JSON.stringify(value, null, 2) + "\n";
    } else {
      const patterns =
        file === "Cargo.lock"
          ? ["proton-drive-desktop", "drive-core"].map(
              (name) =>
                new RegExp(`(name = "${name}"\\nversion = ")([^"]+)(")`),
            )
          : [/^(version = ")([^"]+)(")/m];
      after = before;
      for (const pattern of patterns) {
        const match = after.match(pattern);
        if (!match) throw new Error(`Missing package version in ${file}`);
        if (check && match[2] !== version)
          throw new Error(`Version differs in ${file}`);
        after = after.replace(
          pattern,
          (_, prefix, _old, suffix) => prefix + version + suffix,
        );
      }
    }
    updates.push([file, after]);
  }
  // Validate every file before writing any of them.
  if (!check)
    for (const [file, value] of updates)
      await writeFile(resolve(cwd, file), value);
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const check = process.argv[2] === "--check";
  const current = JSON.parse(await readFile("package.json", "utf8")).version;
  await setVersion(check ? current : process.argv[2], process.cwd(), check);
  console.log(
    check ? `Versions agree: ${current}` : `Version set to ${process.argv[2]}`,
  );
}
