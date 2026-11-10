import init, { Viewer } from "./pkg/skald_view.js";
const ready = init();
let viewer;
self.onmessage = async ({ data }) => {
  const { id, type } = data;
  try {
    const wasm = await ready;
    let result;
    if (type === "open") {
      viewer?.free();
      viewer = new Viewer(new Uint8Array(data.buffer));
      result = JSON.parse(viewer.metadata());
    } else if (type === "render") {
      if (!viewer) throw Error("Open a document first");
      const start = performance.now();
      const rgba = viewer.render(data.page, data.scale);
      const info = JSON.parse(viewer.frame_info());
      result = {
        ...info,
        rgba: rgba.buffer,
        renderMs: performance.now() - start,
        cachedBytes: viewer.cached_bytes(),
        frameCacheBytes: viewer.frame_cache_bytes(),
        wasmLinearBytes: wasm.memory.buffer.byteLength,
      };
    } else throw Error("Unknown worker operation");
    self.postMessage({ id, result }, result.rgba ? [result.rgba] : []);
  } catch (e) {
    self.postMessage({ id, error: e.message ?? String(e) });
  }
};
