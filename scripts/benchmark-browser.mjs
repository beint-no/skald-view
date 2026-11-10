import { chromium } from "@playwright/test";
import { mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { gzipSync, brotliCompressSync } from "node:zlib";
import os from "node:os";
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  await page.goto(
    `${process.env.SKALD_VIEW_URL ?? "http://127.0.0.1:4173"}/web/benchmark.html`,
  );
  await page.waitForFunction(() => window.benchmarkReady);
  const result = await page.evaluate(() => window.runBenchmarks(9));
  result.environment = {
    cpu: os.cpus()[0].model,
    platform: os.platform(),
    arch: os.arch(),
    node: process.version,
    pdfjs: JSON.parse(readFileSync("node_modules/pdfjs-dist/package.json"))
      .version,
  };
  result.sizes = {};
  result.sizesOrigin = new URL(page.url()).origin;
  for (const path of [
    "web/pkg/skald_view_bg.wasm",
    "web/pkg/skald_view.js",
    "node_modules/pdfjs-dist/build/pdf.mjs",
    "node_modules/pdfjs-dist/build/pdf.worker.mjs",
  ]) {
    const response = await page.request.get(
      new URL(path, result.sizesOrigin + "/").href,
    );
    if (!response.ok()) throw Error(`Unable to measure served asset: ${path}`);
    const bytes = await response.body();
    result.sizes[path] = {
      raw: bytes.length,
      gzip: gzipSync(bytes).length,
      brotli: brotliCompressSync(bytes).length,
    };
  }
  mkdirSync("bench-results", { recursive: true });
  writeFileSync(
    "bench-results/browser.json",
    JSON.stringify(result, null, 2) + "\n",
  );
  const median = (a) => [...a].sort((a, b) => a - b)[Math.floor(a.length / 2)];
  console.log("fixture,engine,median_first_page_ms,median_warm_page_ms");
  for (const fixture of [...new Set(result.records.map((r) => r.fixture))])
    for (const engine of ["skald", "pdfjs-display", "pdfjs-print"]) {
      const rows = result.records.filter(
        (r) => r.fixture === fixture && r.engine === engine,
      );
      console.log(
        [
          fixture,
          engine,
          median(rows.map((r) => r.firstPageMs)).toFixed(3),
          median(rows.map((r) => r.warmPageMs)).toFixed(3),
        ].join(","),
      );
    }
  console.log(JSON.stringify(result.sizes, null, 2));
} finally {
  await browser.close();
}
