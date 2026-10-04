import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  mkdtemp,
  mkdir,
  copyFile,
  readFile,
  writeFile,
  rm,
  symlink,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import { planRelease as createPlan } from "../scripts/release/plan.mjs";
import { setVersion, releaseFiles } from "../scripts/release/version.mjs";
import {
  restoreEmbeddedExecutables,
  verifyEmbeddedExecutables,
  prepareDesktopMetadata,
  verifyDesktopMetadata,
  verifyZsync,
  updateInformation,
  verifyBundledNotices,
} from "../scripts/release/appimage.mjs";

const project = fileURLToPath(new URL("../", import.meta.url));
const planRelease = (cwd) =>
  createPlan(cwd, "https://github.com/example/capydock");
const git = (cwd, ...args) =>
  execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();

async function embeddedFixture(t) {
  const cwd = await mkdtemp(resolve(tmpdir(), "capydock-embedded-test-"));
  t.after(() => rm(cwd, { recursive: true, force: true }));
  const appDir = resolve(cwd, "CapyDock.AppDir");
  const embedded = resolve(appDir, "usr/lib/CapyDock/bin");
  await mkdir(resolve(cwd, "bin"));
  await mkdir(embedded, { recursive: true });
  for (const [binary, metadata] of [
    ["proton-drive", "release.json"],
    ["proton-drive-computers", "computers-release.json"],
  ]) {
    const bytes = Buffer.from(`original-${binary}`);
    const info = JSON.stringify({
      version: "0.1.0",
      sha512: createHash("sha512").update(bytes).digest("hex"),
    });
    await writeFile(resolve(cwd, "bin", binary), bytes);
    await writeFile(resolve(cwd, "bin", metadata), info);
    await writeFile(resolve(embedded, binary), `${bytes}-rewritten-rpath`);
    await writeFile(resolve(embedded, metadata), info);
  }
  return { cwd, appDir, embedded };
}

test("packaging rejects missing or changed code, artwork and third party notices", async (t) => {
  const { cwd, appDir } = await embeddedFixture(t);
  const notices = ["LICENSE", "ASSETS_LICENSE.md", "THIRD_PARTY_NOTICES.md"];
  for (const notice of notices) {
    await copyFile(resolve(project, notice), resolve(cwd, notice));
    await copyFile(
      resolve(project, notice),
      resolve(appDir, "usr/lib/CapyDock", notice),
    );
  }
  await verifyBundledNotices(appDir, cwd);
  for (const notice of notices) {
    const bundled = resolve(appDir, "usr/lib/CapyDock", notice);
    await writeFile(bundled, "Unexpected notice");
    await assert.rejects(
      verifyBundledNotices(appDir, cwd),
      /Bundled project notice/,
    );
    await rm(bundled);
    await assert.rejects(verifyBundledNotices(appDir, cwd), /ENOENT/);
    await copyFile(resolve(project, notice), bundled);
  }
  await verifyBundledNotices(appDir, cwd);
});

test("detects linuxdeploy changes and restores both embedded executables exactly", async (t) => {
  const { cwd, appDir, embedded } = await embeddedFixture(t);
  await assert.rejects(
    verifyEmbeddedExecutables(appDir, cwd),
    /Embedded executable/,
  );
  await restoreEmbeddedExecutables(appDir, cwd);
  const info = await verifyEmbeddedExecutables(appDir, cwd);
  assert.deepEqual(Object.keys(info), ["protonCli", "computersHelper"]);
  await writeFile(
    resolve(embedded, "proton-drive-computers"),
    "corrupted helper",
  );
  await assert.rejects(
    verifyEmbeddedExecutables(appDir, cwd),
    /proton-drive-computers/,
  );
  await restoreEmbeddedExecutables(appDir, cwd);
  await writeFile(resolve(embedded, "release.json"), "{}");
  await assert.rejects(verifyEmbeddedExecutables(appDir, cwd), /proton-drive/);
});

test("rejects a modified source before replacing any embedded executable", async (t) => {
  const { cwd, appDir, embedded } = await embeddedFixture(t);
  const before = await readFile(resolve(embedded, "proton-drive"));
  await writeFile(
    resolve(cwd, "bin/proton-drive-computers"),
    "unexpected bytes",
  );
  await assert.rejects(
    restoreEmbeddedExecutables(appDir, cwd),
    /Source executable/,
  );
  assert.deepEqual(await readFile(resolve(embedded, "proton-drive")), before);
});

test("AppImage has real CapyDock icons and current metadata without changing its launcher", async (t) => {
  const { cwd, appDir } = await embeddedFixture(t);
  await mkdir(resolve(cwd, "src-tauri/icons"), { recursive: true });
  for (const icon of ["icon.png", "128x128.png", "32x32.png"])
    await copyFile(
      resolve(project, "src-tauri/icons", icon),
      resolve(cwd, "src-tauri/icons", icon),
    );
  await writeFile(
    resolve(appDir, "CapyDock.desktop"),
    "[Desktop Entry]\nName=Proton Drive\nIcon=proton-drive-desktop\nExec=proton-drive-desktop %U\nX-AppImage-Version=0.0.1\n\n[Desktop Action Open]\nName=Open\nExec=proton-drive-desktop\n",
  );
  await symlink("missing.png", resolve(appDir, ".DirIcon"));
  await prepareDesktopMetadata(appDir, cwd, "0.1.2");
  await verifyDesktopMetadata(appDir, cwd, "0.1.2");
  const desktop = await readFile(resolve(appDir, "CapyDock.desktop"), "utf8");
  assert.match(desktop, /^Exec=proton-drive-desktop %U$/m);
  assert.match(
    desktop,
    /\[Desktop Action Open\]\nName=Open\nExec=proton-drive-desktop/,
  );
  assert.doesNotMatch(desktop, /Icon=proton-drive-desktop/);
  await assert.rejects(
    verifyDesktopMetadata(appDir, cwd, "0.1.3"),
    /X-AppImage-Version/,
  );
  await writeFile(resolve(appDir, ".DirIcon"), "capydock.png");
  await assert.rejects(
    verifyDesktopMetadata(appDir, cwd, "0.1.2"),
    /CapyDock PNG/,
  );
});

test("AppImage updates follow this repository's stable x86-64 releases", () => {
  assert.equal(
    updateInformation("ItsAnunesS/capydock"),
    "gh-releases-zsync|ItsAnunesS|capydock|latest|CapyDock_*_x86_64.AppImage.zsync",
  );
  for (const repository of [
    "",
    "owner/repo/extra",
    "owner/repo|other",
    "https://github.com/owner/repo",
  ])
    assert.throws(() => updateInformation(repository), /owner\/repository/);
});

test("zsync references the final release filename and the exact AppImage bytes", async (t) => {
  const { cwd } = await embeddedFixture(t);
  const name = "CapyDock_0.1.2_x86_64.AppImage";
  const image = resolve(cwd, name);
  const control = `${image}.zsync`;
  const bytes = Buffer.from("AppImage contents");
  await writeFile(image, bytes);
  const header = `zsync: 0.6.2\nFilename: ${name}\nURL: ${name}\nLength: ${bytes.length}\nSHA-1: ${createHash("sha1").update(bytes).digest("hex")}\n\n`;
  await writeFile(
    control,
    Buffer.concat([Buffer.from(header), Buffer.from([1, 2, 3])]),
  );
  await verifyZsync(image, control);
  await assert.rejects(
    verifyZsync(image, control, "CapyDock.AppImage"),
    /Filename/,
  );
  await writeFile(
    control,
    header.replace(`URL: ${name}`, "URL: /tmp/build/CapyDock.AppImage") +
      "data",
  );
  await assert.rejects(verifyZsync(image, control), /URL/);
  await writeFile(control, header);
  await assert.rejects(verifyZsync(image, control), /control file/);
  await writeFile(control, header + "data");
  await writeFile(image, "AppImage changed!");
  await assert.rejects(verifyZsync(image, control), /SHA-1/);
});

async function fixture(t) {
  const base = await mkdtemp(resolve(tmpdir(), "capydock-release-test-"));
  t.after(() => rm(base, { recursive: true, force: true }));
  const cwd = resolve(base, "repo");
  await mkdir(cwd);
  git(cwd, "init", "-b", "dev");
  git(cwd, "config", "user.name", "Release Test");
  git(cwd, "config", "user.email", "release-test@example.invalid");
  git(cwd, "config", "commit.gpgsign", "false");
  git(cwd, "config", "tag.gpgsign", "false");
  for (const file of releaseFiles) {
    await mkdir(dirname(resolve(cwd, file)), { recursive: true });
    await copyFile(resolve(project, file), resolve(cwd, file));
  }
  await setVersion("0.1.0", cwd);
  git(cwd, "add", ".");
  git(cwd, "commit", "-m", "feat!: initialize the application");
  return { cwd, base };
}

test("first release is exactly 0.1.0, including when initial commits are breaking", async (t) => {
  const { cwd } = await fixture(t);
  const plan = await planRelease(cwd);
  assert.equal(plan.version, "0.1.0");
  assert.equal(plan.tag, "v0.1.0");
  assert.equal(plan.publish, true);
  assert.match(plan.notes, /0\.1\.0/);
});

for (const [message, expected] of [
  ["fix: recover a missing folder", "0.1.1"],
  ["perf: reduce metadata requests", "0.1.1"],
  ["feat: add another provider", "0.2.0"],
  ["feat!: change configuration format", "1.0.0"],
  [
    "refactor: replace configuration\n\nBREAKING CHANGE: old configurations are no longer supported",
    "1.0.0",
  ],
])
  test(`Conventional Commits: ${message.split("\n")[0]}`, async (t) => {
    const { cwd } = await fixture(t);
    git(cwd, "tag", "v0.1.0");
    git(cwd, "commit", "--allow-empty", "-m", message);
    assert.equal((await planRelease(cwd)).version, expected);
  });

test("documentation-only changes do not publish a release", async (t) => {
  const { cwd } = await fixture(t);
  git(cwd, "tag", "v0.1.0");
  git(cwd, "commit", "--allow-empty", "-m", "docs: explain startup");
  const plan = await planRelease(cwd);
  assert.equal(plan.version, "0.1.0");
  assert.equal(plan.publish, false);
});

test("unrelated tags cannot change this branch's version", async (t) => {
  const { cwd } = await fixture(t);
  const other = git(cwd, "commit-tree", "HEAD^{tree}", "-m", "unrelated root");
  git(cwd, "tag", "v99.0.0", other);
  assert.equal((await planRelease(cwd)).version, "0.1.0");
});

test("all app versions update together; invalid values and drift are rejected", async (t) => {
  const { cwd } = await fixture(t);
  await setVersion("0.2.3", cwd);
  await setVersion("0.2.3", cwd, true);
  await assert.rejects(setVersion("01.0.0", cwd));
  await assert.rejects(setVersion("0.2.3-beta", cwd));
  const file = resolve(cwd, "package.json");
  const data = JSON.parse(await readFile(file, "utf8"));
  data.version = "0.2.4";
  await writeFile(file, JSON.stringify(data));
  await assert.rejects(setVersion("0.2.3", cwd, true));
});

async function publishingFixture(t) {
  const context = await fixture(t);
  const { cwd, base } = context;
  const remote = resolve(base, "remote.git");
  git(base, "init", "--bare", remote);
  git(cwd, "remote", "add", "origin", remote);
  git(cwd, "branch", "release");
  git(cwd, "push", "origin", "dev", "release");
  git(cwd, "switch", "release");
  const plan = await planRelease(cwd);
  const output = resolve(cwd, "release-dist");
  await mkdir(resolve(output, "assets"), { recursive: true });
  await writeFile(resolve(output, "release-plan.json"), JSON.stringify(plan));
  await writeFile(resolve(output, "RELEASE_NOTES.md"), plan.notes);
  let sums = "";
  for (const [name, content] of [
    [`CapyDock_${plan.version}_x86_64.AppImage`, "test AppImage"],
    [`CapyDock_${plan.version}_x86_64.AppImage.zsync`, "test zsync"],
    ["build-info.json", "{}"],
  ]) {
    await writeFile(resolve(output, "assets", name), content);
    sums += `${createHash("sha256").update(content).digest("hex")}  ${name}\n`;
  }
  await writeFile(resolve(output, "assets/SHA256SUMS"), sums);
  for (const file of releaseFiles) {
    await mkdir(dirname(resolve(output, "source", file)), { recursive: true });
    await copyFile(resolve(cwd, file), resolve(output, "source", file));
  }
  const commands = resolve(base, "commands");
  await mkdir(commands);
  await writeFile(
    resolve(commands, "gh"),
    `#!/usr/bin/env node
const fs = require('node:fs');
const path = process.env.RELEASE_TEST_STATE;
const args = process.argv.slice(2);
fs.appendFileSync(path + '.log', args.join(' ') + '\\n');
if (args[1] === 'view') {
  if (!fs.existsSync(path)) process.exit(1);
  process.stdout.write(fs.readFileSync(path));
} else if (args[1] === 'create') fs.writeFileSync(path, JSON.stringify({ isDraft: true }));
else if (args[1] === 'upload' && process.env.RELEASE_TEST_FAIL_UPLOAD) process.exit(1);
else if (args[1] === 'edit') fs.writeFileSync(path, JSON.stringify({ isDraft: false }));
`,
    { mode: 0o755 },
  );
  const env = {
    ...process.env,
    PATH: `${commands}:${process.env.PATH}`,
    GITHUB_SHA: plan.commit,
    GITHUB_REF: "refs/heads/release",
    GITHUB_REPOSITORY: "ItsAnunesS/capydock",
    RELEASE_TEST_STATE: resolve(base, "github-release.json"),
  };
  const publish = (extra = {}) =>
    spawnSync(
      process.execPath,
      [resolve(project, "scripts/release/publish.mjs")],
      { cwd, env: { ...env, ...extra }, encoding: "utf8" },
    );
  return { ...context, remote, plan, env, publish };
}

test("publishes release and its tag without changing dev or overwriting published assets", async (t) => {
  const { cwd, plan, env, publish } = await publishingFixture(t);
  const result = publish();
  assert.equal(result.status, 0, result.stderr);
  const tag = git(cwd, "rev-parse", `${plan.tag}^{}`);
  assert.equal(
    git(cwd, "ls-remote", "origin", "refs/heads/release").split(/\s/)[0],
    tag,
  );
  assert.equal(
    git(cwd, "ls-remote", "origin", "refs/heads/dev").split(/\s/)[0],
    plan.commit,
  );
  assert.match(
    git(cwd, "log", "-1", "--format=%s"),
    /^chore\(release\): v0\.1\.0/,
  );
  const retry = publish();
  assert.equal(retry.status, 0, retry.stderr);
  assert.match(retry.stdout, /already published/);
  const log = await readFile(`${env.RELEASE_TEST_STATE}.log`, "utf8");
  assert.equal(
    log.split("\n").filter((line) => line.startsWith("release upload")).length,
    1,
  );
});

test("a failed asset upload resumes the same draft and tag on retry", async (t) => {
  const { cwd, plan, env, publish } = await publishingFixture(t);
  assert.notEqual(publish({ RELEASE_TEST_FAIL_UPLOAD: "1" }).status, 0);
  const tag = git(cwd, "rev-parse", `${plan.tag}^{}`);
  git(cwd, "checkout", "--detach", plan.commit);
  const retry = publish();
  assert.equal(retry.status, 0, retry.stderr);
  assert.equal(git(cwd, "rev-parse", `${plan.tag}^{}`), tag);
  assert.equal(
    JSON.parse(await readFile(env.RELEASE_TEST_STATE, "utf8")).isDraft,
    false,
  );
});

test("a newer release commit is preserved and an obsolete build is not published", async (t) => {
  const { cwd, plan, publish } = await publishingFixture(t);
  git(cwd, "commit", "--allow-empty", "-m", "fix: newer change");
  const newer = git(cwd, "rev-parse", "HEAD");
  git(cwd, "push", "origin", "HEAD:release");
  git(cwd, "checkout", "--detach", plan.commit);
  const result = publish();
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /release advanced/);
  assert.equal(
    git(cwd, "ls-remote", "origin", "refs/heads/release").split(/\s/)[0],
    newer,
  );
  assert.equal(git(cwd, "tag", "-l", plan.tag), "");
});

test("dev and pull request refs cannot publish even with a valid release artifact", async (t) => {
  const { cwd, publish, plan } = await publishingFixture(t);
  for (const ref of ["refs/heads/dev", "refs/pull/1/merge"]) {
    const result = publish({ GITHUB_REF: ref });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /Only release may publish/);
  }
  assert.equal(git(cwd, "tag", "-l", plan.tag), "");
  assert.equal(
    git(cwd, "ls-remote", "origin", "refs/heads/release").split(/\s/)[0],
    plan.commit,
  );
});

test("independent changes on dev do not prevent a release or get overwritten", async (t) => {
  const { cwd, plan, publish } = await publishingFixture(t);
  git(cwd, "switch", "dev");
  git(cwd, "commit", "--allow-empty", "-m", "feat: unreleased development");
  const dev = git(cwd, "rev-parse", "HEAD");
  git(cwd, "push", "origin", "dev");
  git(cwd, "checkout", "--detach", plan.commit);
  const result = publish();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(
    git(cwd, "ls-remote", "origin", "refs/heads/dev").split(/\s/)[0],
    dev,
  );
  assert.equal(
    git(cwd, "ls-remote", "origin", "refs/heads/release").split(/\s/)[0],
    git(cwd, "rev-parse", `${plan.tag}^{}`),
  );
});

test("changed artifacts and mismatched source commits block publication", async (t) => {
  const { cwd, plan, publish } = await publishingFixture(t);
  assert.notEqual(publish({ GITHUB_SHA: "a".repeat(40) }).status, 0);
  await writeFile(
    resolve(
      cwd,
      `release-dist/assets/CapyDock_${plan.version}_x86_64.AppImage`,
    ),
    "modified",
  );
  const result = publish();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Checksum mismatch/);
  assert.equal(git(cwd, "tag", "-l", plan.tag), "");
});

test("a missing or corrupted zsync prevents publishing a broken update channel", async (t) => {
  const { cwd, plan, publish } = await publishingFixture(t);
  const path = resolve(
    cwd,
    `release-dist/assets/CapyDock_${plan.version}_x86_64.AppImage.zsync`,
  );
  await writeFile(path, "corrupted update metadata");
  const corrupt = publish();
  assert.notEqual(corrupt.status, 0);
  assert.match(corrupt.stderr, /Checksum mismatch/);
  await rm(path);
  const missing = publish();
  assert.notEqual(missing.status, 0);
  assert.match(missing.stderr, /Unexpected release assets/);
  assert.equal(git(cwd, "tag", "-l", plan.tag), "");
});
