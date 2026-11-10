# Skald View

A new PDF 2.0 viewer for controlled documents. Rust + WebAssembly, a worker-based
browser library, Brotli streams, and experimental JPEG XL images. Apache-2.0.

**1.0.0 implements the bounded `skald-pdf2-v1` profile for controlled Skald documents.**
Its stable native/worker API and embedding archive are ready for a local ReAI trial.
The [readiness report](docs/readiness-1.0.0.md) records coverage, exclusions and
measured tradeoffs. ReAI production is unchanged; JPEG XL embedding remains experimental.

## Try it

Install Rust (the pinned toolchain is in `rust-toolchain.toml`) and Node.js 26+:

```sh
cargo install wasm-bindgen-cli --version 0.2.130 --locked
npm ci
npm run build
npm run dev
```

Open **http://127.0.0.1:4173**. The app includes **31 redistributable examples**:
Brotli invoices and reports, multilingual selectable text, RGB/gray JPEG XL at
lossless and lossy qualities, reversible JPEG recompression, soft masks,
clipping, shared images and an intentional profile rejection. Local files and
drag-and-drop stay in the browser; nothing is uploaded.

## Architecture

The PDF parser, strict interpreter, immutable page scenes and browser client are
new code. Cross-reference streams and object streams are required; there is no
classic-xref reader, malformed-file repair, PDF 1.x path, PDF JavaScript,
PDF.js runtime dependency or native PDF engine.

- **Rust/WASM + SIMD**, with no unsafe code in this project's engine.
- **Vello CPU 0.3** sparse-strip rasterization, shared native/WASM implementation.
- **jxl-rs 0.8** JPEG XL decoding and pure-Rust Brotli decoding.
- Embedded TrueType glyph outlines at PDF glyph positions; no substitute fonts
  or browser reshaping in the visual renderer.
- On-demand page compilation, bounded scene LRU and shared decoded assets.
- Worker isolation, a hard render deadline and a bounded, latest-request queue.
- Canvas presentation with a selectable text overlay, page search, navigation,
  keyboard controls, zoom and a responsive fixture gallery.

[Research and technology decisions](docs/architecture.md) ·
[Supported profile and limits](docs/profile.md) ·
[Measurements and methodology](docs/benchmarks.md) · [Security](SECURITY.md)

JPEG XL embedding uses provisional `/JXLDecode` semantics matching Skald PDF's
experimental output. It is not a finalized PDF interoperability contract.

## Release archive

Download `skald-view-1.0.0-web.tar.gz` and its SHA-256 sidecar from
[the release](https://github.com/beint-no/skald-view/releases/tag/v1.0.0).
The archive is served as a unit; its `embed.html` uses a same-origin postMessage
protocol documented in [the embedding guide](docs/embedding.md).

## Browser library

Serve `web/client.mjs`, `web/scheduler.mjs`, `web/worker.mjs` and the generated
`web/pkg/` directory together. No framework or cross-origin-isolation headers
are required. A modern browser with WASM SIMD and module workers is required.

```js
import { SkaldDocument } from './web/client.mjs';
const document = new SkaldDocument();
const pages = await document.open(arrayBuffer); // transfers ownership
const frame = await document.render(0, 1.5);    // zero-based page, pixels/point
// frame: width, height, rgba ArrayBuffer, text, renderMs, cachedBytes
canvas.width = frame.width;
canvas.height = frame.height;
canvas.getContext('2d').putImageData(
  new ImageData(new Uint8ClampedArray(frame.rgba), frame.width, frame.height), 0, 0);
document.close();
```

Do not send this profile arbitrary production PDFs without validating their
features first. The renderer returns explicit errors for unsupported content.
See the example app for lifecycle management, stale-result rejection and
selection overlay construction.

## Native library

```rust,no_run
let mut doc = skald_view::Document::open(bytes, skald_view::Limits::default())?;
let frame = doc.render(0, 1.0)?;
```

`cargo run --release --example render -- input.pdf output.png [page-index]`
provides a native inspection tool. Nothing is published to crates.io or npm yet;
the 1.0 API follows the explicit supported profile.

## Test and benchmark

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
npm test
npx playwright install --with-deps
npm run build
npm run test:browser
npm run verify:corpus  # requires mutool
cargo bench --bench pipeline
# With npm run dev running in another terminal:
npm run benchmark:browser
```

Tests include independent MuPDF golden images, exact Flate/Brotli and lossless
JXL pixel comparisons, genuinely lossy JXL tests, Unicode, soft masks, cache
reuse/eviction, malformed input boundaries and 4,000 generated parser cases.
Playwright exercises Chromium, Firefox and WebKit. PDF.js is a **dev-only
comparison baseline**, not a rendering fallback.

Timing results are not CI assertions. The original PDF.js comparison showed faster
first-page completion on the small tested corpus; PDF.js's print path is faster
on several warm vector/text cases. This is not universal superiority. Full
methodology, differences and download-size costs are in [the benchmark report](docs/benchmarks.md).
The [1.0.0 before/after report](docs/readiness-1.0.0.md) separately records lower
retention and faster exact-page revisits, with a small cold-render regression.

## Fixture provenance

The original example documents are fictional and generated by
[`examples/GenerateFixtures.java`](examples/GenerateFixtures.java) using Skald
PDF 1.21.0+. No customer documents are included. To regenerate, build the Skald
modules and run `node scripts/generate-fixtures.mjs /path/to/skald-pdf` with the
native Brotli/JXL libraries available. Independent reference images were rendered
using MuPDF 1.28.6 at 72 DPI; they are not Skald View self-generated goldens.
The embedded IBM Plex Sans subsets are covered by the bundled [OFL](docs/licenses/IBM-Plex-OFL-1.1.txt).
The additional application corpus is pinned separately in
[`tests/skald-corpus/manifest.json`](tests/skald-corpus/manifest.json), with its
Skald source revision, generation command and checksums.
