# Research and architecture decision

Decision date: 10 November 2026. Initial target: ReAI's future, controlled Skald
PDF output, including invoices, reports and normalized scanned documents. No
ReAI deployment or consumer integration is part of this repository.

## Evaluated approaches

| Approach | Strength | Reason for this decision |
| --- | --- | --- |
| PDF.js with codec additions | Mature browser integration and usability | Carries a broad PDF compatibility surface; retained as an independent dev baseline |
| PDFium/MuPDF in WASM | Mature rendering | General-purpose native engines and compatibility surface exceed the target |
| Hayro | Modern pure-Rust PDF interpretation | Experimental, broad coverage; upstream explicitly prioritizes feature completeness over optimization at present |
| New restricted PDF interpreter + modern Rust rasterizer | Explicit semantics, small scope, controlled caches | Chosen; we own PDF interpretation and can iterate codecs without browser release dependencies |
| New codec/font/rasterizer implementations too | Total ownership | Would duplicate specialized, reviewed work without evidence of benefit |

The new code does not wrap another PDF renderer. It reuses lower-level codecs,
TrueType parsing and rasterization, as a modern implementation should.

## Why Rust and Vello CPU

Rust provides native and WASM builds from one engine, explicit ownership for
shared stream/scene data and a strong basis for safe untrusted-input handling.
This project's code forbids unsafe Rust. That does not mean every dependency is
unsafe-free, nor does memory safety prevent resource exhaustion.

Vello CPU's sparse-strip architecture and WASM SIMD support fit reproducible,
on-demand document rendering. Its current upstream documentation describes CPU
as the most mature Vello backend. The compute-centric renderer remains
experimental; GPU acceleration is not automatically a win once image decoding,
scene preparation, uploads and readback are included. A future GPU backend can
consume the same retained page scene after browser measurements justify it.

JPEG XL uses the JPEG XL project's Rust implementation, jxl-rs, with sample
limits applied before pixel decoding. Brotli uses the pure-Rust
brotli-decompressor state machine; bounded output and trailing-data checks are
ours. Font glyph outlines are taken from the embedded TrueType program and
positioned using PDF CID widths, not reshaped as new browser text.

## Data flow and ownership

```
PDF bytes → immutable shared source + xref index
          → lazy indirect/object-stream resolution
          → bounded stream/image decoding
          → strict content interpreter + embedded glyph outlines
          → retained immutable page scene + Unicode runs
          → Vello CPU (WASM SIMD) → owned RGBA buffer
          → transferable worker message → canvas + selectable text overlay
```

No eager compilation of offscreen pages is needed. Scene entries have an LRU;
when space is needed old scenes are removed, followed by unreferenced decoded
assets. Shared images are decoded once while retained. The budget includes
actual cached stream/pixel bytes and estimated scene storage, not every heap
allocation. WASM memory has a separate 512 MiB maximum. The worker client has
one active render and one replaceable pending render, plus a deadline that can
terminate a stuck worker. There are no shared-memory/COOP/COEP requirements.

## Sources consulted

- [Vello renderer choices](https://github.com/linebender/vello) and
  [Vello CPU API, maturity and SIMD](https://docs.rs/vello_cpu/0.3.0/vello_cpu/).
- [Sparse-strip rasterization thesis](https://ethz.ch/content/dam/ethz/special-interest/infk/inst-pls/plf-dam/documents/StudentProjects/MasterTheses/2025-Laurenz-Thesis.pdf).
- [jxl-rs design and safety](https://github.com/libjxl/jxl-rs) and its pinned 0.8 API.
- [Rust Brotli decoder](https://docs.rs/brotli-decompressor/6.3.1/brotli_decompressor/).
- [Hayro's scope and status](https://docs.rs/hayro/0.9.0/hayro/).
- [PDF.js embedding API](https://mozilla.github.io/pdf.js/examples/) and the pinned
  6.5.438 package source (including display-vs-print scheduling).
- [PDF 2.0 and errata](https://pdfa.org/resource/iso-32000-2),
  [Brotli extension](https://pdfa.org/resource/extension-brotli).

These sources inform the design. Their performance claims are not measurements
of this project; our results and limitations are reported separately.

## Next priorities

1. Expand independent correctness cases from actual, anonymized ReAI-generated
   output before any integration. Preserve original uploaded/signed files.
2. Improve accessibility reading order and selection geometry; visual text is
   correct glyph outlines, while the DOM overlay is an approximation.
3. Add tiled high-zoom rendering and browser-level memory/interaction profiling.
4. Add only required document features with independent references and explicit
   tests. Reconcile JXL's final PDF embedding specification when available.
5. Evaluate a Vello GPU backend against CPU on real target devices, keeping both
   only if the measured benefit warrants the complexity.
