# Skald View profile v0

This is a restricted PDF 2.0 viewing profile, not a complete PDF 2.0 consumer.
The parser does not repair input or fall back to another engine. A page that
fails interpretation is not presented as a successful partial rendering.

## Supported

- PDF 2.0 header, zero-generation indirect objects, xref streams and object streams.
- Page trees with inherited resources and MediaBox; direct page CropBox.
- Flate and Brotli stream filters, bounded filter chains, TIFF and PNG predictors
  with 8-bit samples. Brotli large-window syntax is accepted up to 24 window bits.
- Embedded Type0 / Identity-H / CIDFontType2 fonts with TrueType programs,
  CID widths, Identity or streamed CIDToGIDMap, and supported ToUnicode bfchar /
  bfrange mappings. Text is rendered from glyph outlines at PDF positions.
- Paths, cubic curves, fill/stroke, even-odd fills/clips, transforms, clipping,
  dashed strokes, caps, joins and Normal alpha compositing.
- Form XObjects without transparency groups or external references.
- RGB8/gray8 raw/predictor images and JPEG; RGB8/gray8 static opaque JXL with
  sRGB/D65 semantics matching the explicit PDF image dictionary.
- External gray soft masks, including on JXL images. The current mask scaler
  uses nearest sampling; high-quality arbitrary-size mask resampling is not claimed.
- Text showing/positioning/spacing and invisible text; marked-content wrappers
  without optional-content behavior.

## Explicitly outside this version

PDF 1.x, classic/hybrid cross-reference tables, revision chains, encryption,
forms, annotations/links, optional content, active actions, page rotation,
inherited CropBox, UserUnit, page/Form transparency groups, non-Normal blending,
shadings/patterns, hairline strokes, text stroke/clip modes, arbitrary CMaps,
nonembedded fonts, CFF/Type1/Type3 fonts, vertical writing, CMYK/ICC image color
management, JPEG 2000, JBIG2, CCITT, inline images, stencil/explicit masks,
nonidentity image Decode arrays, embedded image alpha, HDR, animation and
high-precision JXL. Unsupported visual operators return errors.

JPEG XL PDF embedding is **experimental**. `/JXLDecode` is provisional. Passing
these examples does not establish finalized standards conformance or support
in other PDF viewers. Native HTML image support is independent of PDF support.

## Default budgets

| Resource | Limit |
| --- | ---: |
| Input | 64 MiB |
| Decoded stream | 16 MiB |
| Tracked cache (including estimated scenes) | 128 MiB |
| Image / rendered page pixels | 16 million each |
| Indexed objects | 100,000 |
| Pages | 2,048 |
| Page operations / glyph budget / path nodes | 500,000 |
| Syntax, resolution, graphics and clip depth | 64 |
| Form recursion | 32 |
| WASM linear memory | 512 MiB maximum |
| Worker request deadline | 15 seconds, configurable in JS client |

The limits are layered safeguards, not a proof of whole-process bounded memory
or CPU use. Native callers must provide their own process isolation and deadlines
for hostile inputs. WASM allocation failure can trap; the browser client reports
worker errors and discards the operation. A separate security/fuzzing review is
required before production use on unrestricted uploads.

## Usability boundary

The example app supports page navigation, zoom, local files, download of the
original, selection/copy and search of the current page. The invisible DOM text
layer scales browser text to PDF run bounds; it is not a complete accessible
reading-order reconstruction. Full-document search, tagged-PDF accessibility,
links, annotation workflows, printing all pages and tile rendering are future
work. No unsupported feature is advertised as finished.
