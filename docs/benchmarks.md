# Benchmarks — 10 November 2026

These are initial measurements on a **small synthetic document corpus**, not a
claim that Skald View is the fastest or most correct PDF engine generally.
Documents, harnesses and raw measurements are retained so results can be challenged.

## Browser comparison

Apple M5 Max, macOS ARM64, Node 26.13.0; pinned PDF.js 6.5.438 and Playwright
Chromium. Two fresh browser processes, two unrecorded warmup iterations per
fixture/engine, then nine measured iterations each. Engine order alternates.
Tables show medians across the 18 recorded samples. Exact user-agent, hardware
concurrency, dimensions, source sizes and every timing are in the raw JSON.
There is no network throttling or cold HTTP-cache simulation.

**First page** starts before creating a fresh engine/worker/document and ends
when page 1 has been written to a canvas. PDF bytes are already in memory. It
includes worker startup, cached-resource fetching, parsing, decoding, rendering
and Skald's worker transfer. Initial import of the benchmark page's JS modules
is excluded. Browser WASM code/resource caches may be warm. This is not cold
navigation time, network latency, compositor presentation or CPU time alone.

**Warm page** rerenders the same already-open page into a fresh canvas, including
Skald's worker roundtrip and canvas write. This is an API/presentation-pipeline
measurement. PDF.js display rendering intentionally schedules animation frames;
its print path is also measured to expose how much of the display result is
scheduling. Neither path is a perfect isolated rasterizer comparison. PDF.js
print may clean up internal state differently. Tests use scale 1 (72 DPI), no
CPU throttling and no device-pixel-ratio multiplication.

### First page, milliseconds

| Fixture | Skald | PDF.js display | PDF.js print |
| --- | ---: | ---: | ---: |
| invoice-flate | 16.00 | 55.30 | 41.80 |
| invoice-brotli | 16.95 | 57.15 | 46.70 |
| unicode-brotli | 15.20 | 51.65 | 44.85 |
| vectors-brotli | 13.70 | 42.95 | 36.45 |
| raster-flate | 18.65 | 41.75 | 40.80 |
| jpeg-flate | 16.40 | 40.80 | 35.55 |
| report-brotli | 19.65 | 58.45 | 48.85 |

### Warm page, milliseconds

| Fixture | Skald | PDF.js display | PDF.js print |
| --- | ---: | ---: | ---: |
| invoice-flate | 1.50 | 12.65 | 0.80 |
| invoice-brotli | 1.85 | 11.65 | 0.90 |
| unicode-brotli | 1.30 | 12.55 | 0.60 |
| vectors-brotli | 1.00 | 15.65 | 0.50 |
| raster-flate | 0.80 | 14.15 | 3.90 |
| jpeg-flate | 0.80 | 16.00 | 0.80 |
| report-brotli | 2.65 | 11.70 | 2.70 |

Skald completed first-page operations faster in these runs. PDF.js print was
faster in several warm text/vector cases. These results do **not** justify a
blanket speed claim; they motivate testing actual ReAI fixtures and target phones.

### Correctness alongside timing

The harness rejects blank results and records raw pixel MAE against Skald for
PDF.js comparisons. Dense report grid lines have a larger difference (about 6.8
byte levels averaged over RGBA) because of stroke coverage/rasterization. Both
outputs were visually inspected; Skald's report differs from an independently
rendered MuPDF reference by about 2.1 byte levels. The golden test threshold is
2.5 across the supplied reference cases, not bit identity across rasterizers.
Neither aggregate MAE nor a nonblank page proves complete rendering correctness.
Text, transforms, clipping and exact lossless codec equivalence have separate tests.
JXL-only fixtures are not used to manufacture a speed win over an engine that
cannot render them. They have correctness tests and native timing probes.

### Download costs

The measured generated WASM is about **2.64 MB raw / 0.67 MB Brotli**, plus
roughly 8.6 KB raw JS bindings and the small client/worker/queue modules.
The baseline's two main JS modules total roughly **3.14 MB raw / 0.53 MB Brotli**.
Additional PDF.js assets are excluded; the embedded-font fixtures do not need
its general font/CMap asset collection. License notices are distributed but not
fetched by the rendering app. The local dev server serves uncompressed files;
compressed sizes are offline estimates, not measured network transfer.

Thus Skald is not smaller over the wire than this baseline for this workload,
although it includes JXL decoding. First-visit codec cost matters and must not
be hidden behind document compression savings.

## Native probe

`cargo bench --bench pipeline` uses an optimized native build, five warmups and
50 measured iterations per fixture/scenario. It includes document creation for
`open-first-page`; `cached-first-page` reuses a compiled scene. Pixel output is
consumed through `black_box`, not PNG encoding. Median and p95 timings and the
tracked cache size are retained. This is a Skald-only probe, not a native engine
comparison. Native and browser milliseconds must not be treated as interchangeable.

## Reproduce

```sh
cargo bench --bench pipeline
npm run build
npm run dev
# Another terminal:
npm run benchmark:browser
```

- [Browser run 1](benchmarks/2026-11-10-browser-run-1.json)
- [Browser run 2](benchmarks/2026-11-10-browser-run-2.json)
- [Native timings](benchmarks/2026-11-10-native.csv)

No CI timing thresholds are imposed. CI runs correctness, property and browser
tests and uploads the native probe for inspection. No private customer data was
used. Real uploaded-document quality, accessibility and low-memory-device testing
remain release gates before any ReAI integration.
