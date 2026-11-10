# 1.0.0 readiness, warm rendering and memory

The target is controlled Skald PDF 2.0 output. This release does not add a
PDF 1.x/classic-xref parser or a native renderer fallback to the library.

## Document correctness

The pinned Skald corpus contains **91 fictional documents and 102 pages**: ReAI-style
invoices, credit notes, paid copies, reminders, signed invoices, payslips, general
ledger reports, stock lists, receipts, shipping labels, barcodes, tables and visual
examples. All render. Every page is independently compared with MuPDF at 72 DPI.
Maximum RGBA mean absolute error is **2.4201** byte levels, below the retained
2.5 threshold. This aggregate metric is a regression aid, not proof of conformance.

The initial engine accepted 86/91 documents. Axial RGB gradients, safe URI links
and readonly invisible signature fields close the five actual corpus gaps.
Signature presence is metadata, not cryptographic verification. Fill/stroke text
and bounded ASCII85 decoding are also covered by reference/regression cases.

Corpus provenance and source commit are in `tests/skald-corpus/manifest.json`.
`npm run verify:corpus` regenerates independent MuPDF references and verifies
all pages. The corpus has no customer documents.

A separate **local-only** sample of 60 stored PDF 2.0 attachments plus a freshly
generated ReAI general-ledger report accepted 56/61 documents. Three require
unsupported fonts, two require classic xrefs. Across 219 accepted pages, median
MAE against MuPDF was 1.80, maximum 2.99; three dense text pages exceeded the
public-corpus 2.5 threshold. The highest-difference page was visually reviewed
without finding a content/layout discrepancy. These are advisory observations,
not a blanket private-corpus pass. No private files, identities, hashes or
screenshots are published. Existing original viewing remains essential in the
local ReAI trial, especially for its much larger PDF 1.x attachment collection.

## What changed in retained memory

Page scenes now share glyph outlines per font program/glyph rather than storing
a transformed outline copy for every text occurrence. A single bounded exact-
frame cache reuses immutable RGBA for the same page and scale; it is discarded
on another key and disabled above its pixel-byte budget. Native callers receive
shared immutable `Arc<[u8]>` pixels. Browser calls still copy/transfer pixels and
write the canvas. This improves repeat-page operations, not rasterizer throughput.

The eight-page report probe renders all pages then repeats page 1 100 times.
Before: tracked cache **14,371,919 bytes**. After: **4,553,471 bytes**, including
a 2,007,328-byte cached frame: **68.3% lower**. Three fresh native process trials
on Apple M5 Max measured median maximum RSS **33,390,592 → 17,563,648 bytes**
(**47.4% lower**). `/usr/bin/time -l` is used on macOS; RSS is process-specific,
not a whole-browser or malicious-input bound. Source bytes and renderer scratch
allocations are additional to the tracked-cache figure.

The native warm probe improves roughly 0.89 → 0.016 ms because it returns shared
cached pixels. That number must not be described as faster rerasterization.
`Limits.frame_cache_bytes` defaults to 8 MiB and can be set to zero; the combined
tracked cache remains bounded. Tests cover pixel equivalence, page/scale keys,
eviction, disabling the frame cache and sharing images across pages.

## Browser before/after

Same Apple M5 Max, pinned Chromium, scale 1, identical public fixtures and
harness. Two fresh browser processes per revision, two unrecorded warmups and
nine recorded iterations per fixture/engine per process. Version order was
before/after then after/before. The baseline is a69248b; the candidate is 1.0.0.
These medians combine 18 samples. An already-running local ReAI dev server was
present during both versions. Full HTTP cold-cache navigation is not measured.

| Fixture | First page before ms | After ms | Warm before ms | After ms |
| --- | ---: | ---: | ---: | ---: |
| invoice-flate | 16.70 | 17.35 | 1.45 | 0.95 |
| invoice-brotli | 17.45 | 18.20 | 2.15 | 0.80 |
| unicode-brotli | 15.85 | 16.35 | 1.20 | 0.70 |
| vectors-brotli | 14.20 | 14.70 | 1.05 | 0.50 |
| raster-flate | 19.10 | 20.80 | 0.80 | 0.50 |
| jpeg-flate | 16.70 | 17.65 | 0.80 | 0.50 |
| report-brotli | 20.50 | 21.60 | 2.65 | 1.15 |

Warm completion improves on these fixtures. First-page completion regresses
slightly; the release trades some cold work for lower retained memory and faster
revisits. No universal speed improvement is claimed. The prior PDF.js display
and print comparisons remain in the raw measurements; their scheduling differs,
and print is competitive on warm cases. See the original benchmark methodology
for canvas/worker timing boundaries and download-cost limitations.

Raw data:

- [Before browser run 1](benchmarks/1.0.0-before-browser-1.json), [run 2](benchmarks/1.0.0-before-browser-2.json)
- [After browser run 1](benchmarks/1.0.0-after-browser-1.json), [run 2](benchmarks/1.0.0-after-browser-2.json)
- [Native memory/retention trials](benchmarks/1.0.0-native-retention.json)
- [All 102 public-page pixel comparisons](benchmarks/1.0.0-skald-corpus-pixels.json)

## Release boundary

1.0.0 stabilizes the bounded `skald-pdf2-v1` profile, immutable native result and
worker/embedding API. JPEG XL embedding stays provisional/experimental.
The versioned browser archive includes bindings, WASM, the embeddable viewer,
licenses, profile and source manifest, with a SHA-256 sidecar. It is built and
tested from its tag. ReAI production and the primary ReAI checkout are unchanged.
