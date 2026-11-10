# Same-origin embedding API 1.0

Serve the complete versioned release directory from the application's own origin.
Create an iframe pointing to `embed.html`. The frame sends
`{type: "skald-view:ready"}`. The parent must verify both `event.origin` and
`event.source === iframe.contentWindow`, then post:

```js
iframe.contentWindow.postMessage({
  type: 'skald-view:open',
  buffer,                        // ArrayBuffer; ownership is transferred
  title: 'Document',
  locale: 'no',                   // no/nb or en
  source: '/authorized/pdf/url'   // optional original-view link
}, location.origin, [buffer]);
```

The child also checks origin and source. It accepts same-origin messages from
its parent only. It reports `skald-view:rendered` (page/pages) or
`skald-view:error` (diagnostic message). User-facing failures are localized and
retain the original-view link. The renderer does not fetch arbitrary PDF URLs
or execute PDF actions. URI link overlays only allow HTTP(S) and use
`noopener noreferrer` in a separate tab.

The parent owns authorization, fetch cancellation, MIME/version dispatch and
original-file access. Keep existing image/older-PDF serving outside this library;
PDF 1.x and unsupported features do not gain support merely by passing bytes here.
Do not replace stored or signed originals with altered viewing copies.

The ordinary JS client (`client.mjs`) returns a frame with `width`, `height`,
transferred `rgba` ArrayBuffer, Unicode `text` runs, `renderMs`, `cachedBytes`,
`frameCacheBytes` and `wasmLinearBytes`. Tracked cache and WASM linear-memory size
measure different things; neither is whole-browser resident memory. The native
`RenderedPage.rgba` is immutable shared `Arc<[u8]>`; exact frame reuse does not
rerasterize. Set `Limits.frame_cache_bytes=0` to disable it.

Host requirements: current WASM SIMD, module workers, JS modules, same-origin
framing, `application/wasm` MIME. A restrictive CSP must permit self-hosted
workers and WASM compilation (`script-src 'self' 'wasm-unsafe-eval'`); the embed
stylesheet/text overlays also require the host's chosen inline-style policy.
