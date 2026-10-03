import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  rename,
  chmod,
  writeFile,
  rm,
} from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve, dirname } from "node:path";
import { execFileSync } from "node:child_process";

const root = fileURLToPath(new URL("../", import.meta.url));
const destination = resolve(root, "bin/proton-drive");
const metadata = resolve(root, "bin/release.json");
const platform =
  process.arch === "arm64" ? "linux/arm64" : "linux/x64-baseline";
if (process.platform !== "linux" || !["x64", "arm64"].includes(process.arch))
  throw new Error("Este instalador suporta Linux x64 e ARM64.");
const request = async (url) => {
  const parsed = new URL(url);
  if (
    parsed.protocol !== "https:" ||
    parsed.hostname !== "proton.me" ||
    !parsed.pathname.startsWith("/download/drive/cli/")
  )
    throw new Error("Origem de download inválida.");
  const response = await fetch(url, {
    redirect: "error",
    signal: AbortSignal.timeout(180_000),
  });
  if (!response.ok) throw new Error(`Download falhou: HTTP ${response.status}`);
  return response;
};
await mkdir(dirname(destination), { recursive: true });
const manifest = await (
  await request("https://proton.me/download/drive/cli/version.json")
).json();
const release = manifest.Releases.find(
  (item) => item.CategoryName === "Stable",
);
const file = release?.Files.find((item) => item.Platform === platform);
if (!file || !/^[a-f0-9]{128}$/i.test(file.Sha512CheckSum))
  throw new Error("Release oficial sem checksum ou plataforma suportada.");
const bytes = Buffer.from(await (await request(file.Url)).arrayBuffer());
const hash = createHash("sha512").update(bytes).digest("hex");
if (hash !== file.Sha512CheckSum.toLowerCase())
  throw new Error("SHA-512 inválido; binário rejeitado.");
const temporary = destination + ".download";
try {
  await writeFile(temporary, bytes, { mode: 0o755 });
  await chmod(temporary, 0o755);
  const version = execFileSync(temporary, ["version"], {
    encoding: "utf8",
    timeout: 20_000,
  }).trim();
  if (!version.includes(release.Version))
    throw new Error("A versão do binário difere do manifesto.");
  await rename(temporary, destination);
  await writeFile(
    metadata,
    JSON.stringify(
      { version: release.Version, platform, url: file.Url, sha512: hash },
      null,
      2,
    ) + "\n",
  );
  console.log(`${version}\nInstalado: ${destination}\nSHA-512 verificado.`);
} finally {
  await rm(temporary, { force: true });
}
