import { SkaldDocument } from "./client.mjs";
const $ = (id) => document.getElementById(id);
const texts = {
  en: {
    previous: "Previous page",
    next: "Next page",
    page: "Page",
    out: "Zoom out",
    in: "Zoom in",
    fit: "Fit width",
    original: "Open original",
    loading: "Loading PDF…",
    failed:
      "This PDF is outside the supported viewing profile. Open the original to view it.",
    signature: "Signature verification is not performed.",
  },
  no: {
    previous: "Forrige side",
    next: "Neste side",
    page: "Side",
    out: "Zoom ut",
    in: "Zoom inn",
    fit: "Tilpass bredde",
    original: "Åpne original",
    loading: "Laster PDF…",
    failed:
      "Denne PDF-en er utenfor støttet visningsprofil. Åpne originalen for å se den.",
    signature: "Signaturen er ikke verifisert.",
  },
};
let doc,
  pages = [],
  page = 0,
  scale = 1,
  epoch = 0,
  renderId = 0,
  language = "en";
const dpr = Math.min(devicePixelRatio || 1, 2);
function dictionary() {
  return texts[language] ?? texts.en;
}
function controls() {
  const enabled = pages.length > 0;
  const t = dictionary();
  $("page").value = page + 1;
  $("page").max = pages.length;
  $("page").disabled = !enabled;
  $("count").textContent = enabled ? `/ ${pages.length}` : "";
  $("zoom").textContent = `${Math.round(scale * 100)}%`;
  for (const id of ["previous", "next", "in", "out"]) {
    $(id).setAttribute("aria-label", t[id]);
    $(id).disabled =
      !enabled ||
      (id === "previous" && page === 0) ||
      (id === "next" && page === pages.length - 1) ||
      (id === "in" && scale >= maximum() - 1e-6) ||
      (id === "out" && scale <= minimum() + 1e-6);
  }
  $("fit").textContent = t.fit;
  $("original").textContent = t.original;
  $("page").setAttribute("aria-label", t.page);
}
function maximum() {
  const p = pages[page];
  return p
    ? Math.min(4, 8 / dpr, Math.sqrt(15e6 / (p.width * p.height)) / dpr)
    : 4;
}
function minimum() {
  return Math.min(0.25, maximum() / 4);
}
function fit() {
  if (pages.length)
    scale = Math.max(
      minimum(),
      Math.min(maximum(), ($("viewport").clientWidth - 30) / pages[page].width),
    );
}
function textLayer(frame) {
  $("text").replaceChildren();
  const context = document.createElement("canvas").getContext("2d");
  for (const run of frame.text) {
    const span = document.createElement("span");
    span.textContent = run.text;
    span.style.font = `${run.font_size}px Arial`;
    context.font = span.style.font;
    const width = context.measureText(run.text).width;
    const stretch = width ? run.width / width : 1;
    const [a, b, c, d, e, f] = run.transform;
    span.style.top = `${-run.font_size * 0.8}px`;
    span.style.transform = `matrix(${(a / dpr) * stretch},${(b / dpr) * stretch},${-c / dpr},${-d / dpr},${e / dpr},${f / dpr})`;
    $("text").append(span);
  }
}
function links() {
  const p = pages[page];
  $("links").replaceChildren();
  for (const link of p.links ?? []) {
    const url = new URL(link.uri);
    if (!["https:", "http:"].includes(url.protocol)) continue;
    const a = document.createElement("a");
    a.href = url.href;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
    a.setAttribute("aria-label", url.href);
    const [x0, y0, x1, y1] = link.rect;
    a.style.left = `${(x0 - p.x) * scale}px`;
    a.style.top = `${(p.y + p.height - y1) * scale}px`;
    a.style.width = `${(x1 - x0) * scale}px`;
    a.style.height = `${(y1 - y0) * scale}px`;
    $("links").append(a);
  }
}
async function render() {
  if (!pages.length) return;
  scale = Math.min(scale, maximum());
  controls();
  const current = ++renderId;
  const opened = epoch;
  $("status").textContent = dictionary().loading;
  try {
    const frame = await doc.render(page, scale * dpr);
    if (current !== renderId || opened !== epoch) return;
    const c = $("canvas");
    c.width = frame.width;
    c.height = frame.height;
    c.style.width = `${frame.width / dpr}px`;
    c.style.height = `${frame.height / dpr}px`;
    c.getContext("2d", { alpha: false }).putImageData(
      new ImageData(
        new Uint8ClampedArray(frame.rgba),
        frame.width,
        frame.height,
      ),
      0,
      0,
    );
    $("paper").style.width = c.style.width;
    $("paper").style.height = c.style.height;
    textLayer(frame);
    links();
    $("paper").hidden = false;
    $("error").hidden = true;
    $("status").textContent =
      `${dictionary().page} ${page + 1} / ${pages.length}${pages[page].signature_fields_present ? " · " + dictionary().signature : ""}`;
    document.documentElement.dataset.ready = "true";
    parent.postMessage(
      { type: "skald-view:rendered", page: page + 1, pages: pages.length },
      location.origin,
    );
  } catch (error) {
    if (error.name !== "AbortError" && current === renderId) failure(error);
  }
}
function failure(error) {
  $("paper").hidden = true;
  $("error").hidden = false;
  $("error").textContent = dictionary().failed;
  $("status").textContent = "";
  parent.postMessage(
    { type: "skald-view:error", message: error.message },
    location.origin,
  );
}
async function open({ buffer, title, locale, source }) {
  const current = ++epoch;
  ++renderId;
  doc?.close();
  doc = new SkaldDocument();
  pages = [];
  page = 0;
  language =
    String(locale ?? "en").startsWith("no") ||
    String(locale ?? "").startsWith("nb")
      ? "no"
      : "en";
  document.documentElement.lang = language;
  document.title = title || "Skald View";
  $("original").hidden = true;
  if (source) {
    const url = new URL(source, location.href);
    if (url.origin === location.origin || url.protocol === "data:") {
      $("original").href = url.href;
      $("original").hidden = false;
    }
  }
  $("paper").hidden = true;
  $("error").hidden = true;
  $("status").textContent = dictionary().loading;
  controls();
  try {
    pages = await doc.open(buffer);
    if (current !== epoch) return;
    fit();
    await render();
  } catch (error) {
    if (current === epoch) failure(error);
  }
}
window.addEventListener("message", (event) => {
  if (
    event.source !== parent ||
    event.origin !== location.origin ||
    event.data?.type !== "skald-view:open"
  )
    return;
  if (!(event.data.buffer instanceof ArrayBuffer)) return;
  open(event.data);
});
function navigate(index) {
  if (!pages.length) return;
  page = Math.max(0, Math.min(pages.length - 1, index));
  $("viewport").scrollTop = 0;
  render();
}
$("previous").onclick = () => navigate(page - 1);
$("next").onclick = () => navigate(page + 1);
$("page").onchange = () => navigate(Number($("page").value) - 1);
$("in").onclick = () => {
  scale = Math.min(maximum(), scale * 1.2);
  render();
};
$("out").onclick = () => {
  scale = Math.max(minimum(), scale / 1.2);
  render();
};
$("fit").onclick = () => {
  fit();
  render();
};
window.addEventListener("keydown", (event) => {
  if (event.target instanceof HTMLInputElement) return;
  if (event.key === "ArrowRight" || event.key === "PageDown") {
    event.preventDefault();
    navigate(page + 1);
  }
  if (event.key === "ArrowLeft" || event.key === "PageUp") {
    event.preventDefault();
    navigate(page - 1);
  }
});
window.addEventListener("pagehide", () => doc?.close());
controls();
$("paper").hidden = true;
parent.postMessage({ type: "skald-view:ready" }, location.origin);

let autoFit = true;
const observer = new ResizeObserver(() => {
  if (autoFit && pages.length) {
    fit();
    render();
  }
});
observer.observe($("viewport"));
for (const id of ["in", "out"]) {
  $(id).addEventListener("click", () => (autoFit = false));
}
$("fit").addEventListener("click", () => (autoFit = true));
