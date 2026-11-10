import {
  readFileSync,
  mkdirSync,
  cpSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { spawnSync, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
const { version } = JSON.parse(readFileSync("package.json"));
const root = `dist/skald-view-${version}`;
rmSync(root, { recursive: true, force: true });
mkdirSync(root, { recursive: true });
for (const file of [
  "client.mjs",
  "scheduler.mjs",
  "worker.mjs",
  "embed.html",
  "embed.mjs",
  "embed.css",
])
  cpSync(`web/${file}`, `${root}/${file}`);
cpSync("web/pkg", `${root}/pkg`, { recursive: true });
for (const file of ["LICENSE", "NOTICE"]) cpSync(file, `${root}/${file}`);
cpSync("docs/profile.md", `${root}/PROFILE.md`);
writeFileSync(
  `${root}/manifest.json`,
  JSON.stringify(
    {
      name: "skald-view",
      version,
      profile: "skald-pdf2-v1",
      sourceCommit: execFileSync("git", ["rev-parse", "HEAD"], {
        encoding: "utf8",
      }).trim(),
      experimentalJpegXl: true,
    },
    null,
    2,
  ) + "\n",
);
const archive = `skald-view-${version}-web.tar.gz`;
const result = spawnSync(
  "tar",
  ["-czf", `dist/${archive}`, "-C", "dist", `skald-view-${version}`],
  { stdio: "inherit" },
);
if (result.status) process.exit(result.status);
const sha = createHash("sha256")
  .update(readFileSync(`dist/${archive}`))
  .digest("hex");
writeFileSync(`dist/${archive}.sha256`, `${sha}  ${archive}\n`);
console.log(`${archive}: ${sha}`);
