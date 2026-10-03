import { execFileSync } from "node:child_process";
import { mkdir, writeFile, appendFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import semver from "semver";
import { analyzeCommits } from "@semantic-release/commit-analyzer";
import { generateNotes } from "@semantic-release/release-notes-generator";
import { setVersion } from "./version.mjs";

export const initialVersion = "0.1.0";
const git = (cwd, ...args) =>
  execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
const preset = { preset: "conventionalcommits" };

export async function planRelease(
  cwd = process.cwd(),
  repositoryUrl = process.env.GITHUB_REPOSITORY
    ? `https://github.com/${process.env.GITHUB_REPOSITORY}`
    : git(cwd, "remote", "get-url", "origin"),
) {
  const commit = git(cwd, "rev-parse", "HEAD");
  const tags = git(cwd, "tag", "--merged", "HEAD")
    .split("\n")
    .filter((tag) => /^v\d+\.\d+\.\d+$/.test(tag) && semver.valid(tag.slice(1)))
    .sort((a, b) => semver.rcompare(a.slice(1), b.slice(1)));
  const previousTag = tags[0];
  const commits = git(
    cwd,
    "log",
    "--format=%H%n%B%x00",
    previousTag ? `${previousTag}..HEAD` : "HEAD",
  )
    .split("\0")
    .map((value) => value.trim())
    .filter(Boolean)
    .map((value) => ({
      hash: value.slice(0, value.indexOf("\n")),
      message: value.slice(value.indexOf("\n") + 1),
    }));
  const type = await analyzeCommits(preset, {
    cwd,
    commits,
    logger: { log() {} },
  });
  // The first public release is explicitly 0.1.0, regardless of the initial commit types.
  const version = previousTag
    ? type
      ? semver.inc(previousTag.slice(1), type)
      : previousTag.slice(1)
    : initialVersion;
  const tag = `v${version}`;
  const publish = !previousTag || Boolean(type);
  const notes = publish
    ? await generateNotes(preset, {
        cwd,
        commits,
        options: { repositoryUrl },
        lastRelease: previousTag ? { gitTag: previousTag } : {},
        nextRelease: { version, gitTag: tag, gitHead: commit },
      })
    : "No release-worthy changes since the last version.\n";
  return {
    version,
    tag,
    commit,
    previousTag: previousTag || null,
    type: previousTag ? type : "initial",
    publish,
    notes,
  };
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  const plan = await planRelease();
  await setVersion(plan.version);
  await mkdir("release-dist", { recursive: true });
  await writeFile(
    "release-dist/release-plan.json",
    JSON.stringify(plan, null, 2) + "\n",
  );
  await writeFile(
    "release-dist/RELEASE_NOTES.md",
    `${plan.notes}\nLinux x86-64 · AppImage\n\nSource: \`${plan.commit}\`\n`,
  );
  if (process.env.GITHUB_OUTPUT)
    await appendFile(
      process.env.GITHUB_OUTPUT,
      `publish=${plan.publish}\nversion=${plan.version}\n`,
    );
  console.log(`${plan.publish ? "Release" : "Build only"}: ${plan.tag}`);
}
