import { spawnSync } from "node:child_process";
import {
  readFileSync,
  readdirSync,
  mkdtempSync,
  writeFileSync,
  mkdirSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createHash } from "node:crypto";
const run = (command, args) => {
  const r = spawnSync(command, args, {
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  if (r.status !== 0) throw Error(r.stderr || r.stdout);
  return r.stdout;
};
const manifest = JSON.parse(readFileSync("tests/skald-corpus/manifest.json"));
for (const entry of manifest.files) {
  const bytes = readFileSync("tests/skald-corpus/" + entry.file);
  if (
    bytes.length !== entry.bytes ||
    createHash("sha256").update(bytes).digest("hex") !== entry.sha256
  )
    throw Error("Corpus changed without updating its manifest: " + entry.file);
}
const directory = mkdtempSync(join(tmpdir(), "skald-corpus-"));
try {
  const actual = join(directory, "actual");
  const reference = join(directory, "reference");
  mkdirSync(reference);
  const records = JSON.parse(
    run("cargo", [
      "run",
      "--locked",
      "--release",
      "--example",
      "corpus",
      "--",
      "tests/skald-corpus",
      actual,
    ]),
  );
  if (records.some((r) => r.error))
    throw Error(JSON.stringify(records.filter((r) => r.error)));
  for (const file of readdirSync("tests/skald-corpus").filter((f) =>
    f.endsWith(".pdf"),
  ))
    run("mutool", [
      "draw",
      "-q",
      "-r",
      "72",
      "-o",
      join(reference, file.slice(0, -4) + "-%d.png"),
      "tests/skald-corpus/" + file,
    ]);
  const comparisons = JSON.parse(
    run("cargo", [
      "run",
      "--locked",
      "--release",
      "--example",
      "compare",
      "--",
      actual,
      reference,
    ]),
  );
  mkdirSync("bench-results", { recursive: true });
  writeFileSync(
    "bench-results/skald-corpus.json",
    JSON.stringify(
      {
        source: manifest.revision,
        documents: records.length,
        pages: comparisons.length,
        comparisons,
      },
      null,
      2,
    ) + "\n",
  );
  const failures = comparisons.filter((p) => p.mae > 2.5);
  if (failures.length)
    throw Error("Independent reference mismatch: " + JSON.stringify(failures));
  console.log(
    `${records.length} documents / ${comparisons.length} pages pass independent MuPDF comparison (MAE <= 2.5).`,
  );
} finally {
  rmSync(directory, { recursive: true, force: true });
}
