import { execFileSync, spawnSync } from "node:child_process";
import { copyFile, readFile, readdir } from "node:fs/promises";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { assertVersion, releaseFiles, setVersion } from "./version.mjs";

const run = (command, args) =>
  execFileSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
const git = (...args) => run("git", args);
const plan = JSON.parse(
  await readFile("release-dist/release-plan.json", "utf8"),
);
assertVersion(plan.version);
if (
  !plan.publish ||
  plan.tag !== `v${plan.version}` ||
  plan.commit !== process.env.GITHUB_SHA ||
  !/^[a-f0-9]{40}$/.test(plan.commit)
)
  throw new Error("Release plan does not match the validated workflow commit.");
if (process.env.GITHUB_REF !== "refs/heads/dev")
  throw new Error("Only dev may publish releases.");
const repository = process.env.GITHUB_REPOSITORY;
if (repository !== "zephyrushq/capydock")
  throw new Error("Unexpected release repository.");
const expected = [
  `CapyDock_${plan.version}_x86_64.AppImage`,
  "SHA256SUMS",
  "build-info.json",
].sort();
const assets = (await readdir("release-dist/assets")).sort();
if (JSON.stringify(assets) !== JSON.stringify(expected))
  throw new Error("Unexpected release assets.");
const sums = await readFile("release-dist/assets/SHA256SUMS", "utf8");
for (const name of expected.filter((file) => file !== "SHA256SUMS")) {
  const hash = createHash("sha256")
    .update(await readFile(resolve("release-dist/assets", name)))
    .digest("hex");
  if (!sums.split("\n").includes(`${hash}  ${name}`))
    throw new Error(`Checksum mismatch: ${name}`);
}
git("fetch", "origin", "+refs/heads/dev:refs/remotes/origin/dev", "--tags");
const tagExists =
  spawnSync("git", ["rev-parse", "--verify", `refs/tags/${plan.tag}`], {
    stdio: "ignore",
  }).status === 0;
if (tagExists) {
  // Resume a failed upload only when this tag belongs to the exact original build.
  if (
    !git("tag", "-l", plan.tag, "--format=%(contents)")
      .split("\n")
      .includes(`Source-Commit: ${plan.commit}`) ||
    git("rev-parse", `${plan.tag}^1`) !== plan.commit
  )
    throw new Error(
      "This version already belongs to another source commit; refusing to replace it.",
    );
} else {
  if (git("rev-parse", "origin/dev") !== plan.commit) {
    console.log(
      "dev advanced during the build; its queued workflow will publish the newer revision.",
    );
    process.exit(0);
  }
  const releaseRef = git("ls-remote", "--heads", "origin", "release");
  if (releaseRef) {
    git("fetch", "origin", "refs/heads/release:refs/remotes/origin/release");
    git("merge-base", "--is-ancestor", "origin/release", "HEAD");
  }
  for (const file of releaseFiles)
    await copyFile(resolve("release-dist/source", file), file);
  await setVersion(plan.version, process.cwd(), true);
  git("config", "user.name", "github-actions[bot]");
  git(
    "config",
    "user.email",
    "41898282+github-actions[bot]@users.noreply.github.com",
  );
  git("add", "--", ...releaseFiles);
  git("commit", "--allow-empty", "-m", `chore(release): ${plan.tag} [skip ci]`);
  git(
    "tag",
    "-a",
    plan.tag,
    "-m",
    `CapyDock ${plan.tag}\n\nSource-Commit: ${plan.commit}`,
  );
  // All refs advance together, without force pushes. Concurrent changes fail safely.
  git(
    "push",
    "--atomic",
    "origin",
    "HEAD:refs/heads/dev",
    "HEAD:refs/heads/release",
    `refs/tags/${plan.tag}`,
  );
}
const view = spawnSync(
  "gh",
  ["release", "view", plan.tag, "--repo", repository, "--json", "isDraft"],
  { encoding: "utf8" },
);
if (view.status === 0 && !JSON.parse(view.stdout).isDraft) {
  console.log(
    `${plan.tag} is already published; its assets will not be overwritten.`,
  );
  process.exit(0);
}
if (view.status !== 0)
  run("gh", [
    "release",
    "create",
    plan.tag,
    "--repo",
    repository,
    "--verify-tag",
    "--draft",
    "--title",
    `CapyDock ${plan.tag}`,
    "--notes-file",
    "release-dist/RELEASE_NOTES.md",
  ]);
run("gh", [
  "release",
  "upload",
  plan.tag,
  "--repo",
  repository,
  "--clobber",
  ...assets.map((file) => resolve("release-dist/assets", file)),
]);
const newestTag = git("tag", "--list", "v*", "--sort=-version:refname")
  .split("\n")
  .find((tag) => /^v\d+\.\d+\.\d+$/.test(tag));
run("gh", [
  "release",
  "edit",
  plan.tag,
  "--repo",
  repository,
  "--draft=false",
  newestTag === plan.tag ? "--latest" : "--latest=false",
]);
console.log(`Published CapyDock ${plan.tag}.`);
