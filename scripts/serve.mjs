import { createServer } from "node:http";
import { createReadStream, statSync } from "node:fs";
import { resolve, extname, sep } from "node:path";
const root = resolve(import.meta.dirname, "..");
const types = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".css": "text/css",
  ".wasm": "application/wasm",
  ".pdf": "application/pdf",
  ".json": "application/json",
  ".png": "image/png",
};
createServer((req, res) => {
  try {
    const path = resolve(
      root,
      "." +
        decodeURIComponent(
          new URL(req.url, "http://localhost").pathname === "/"
            ? "/web/index.html"
            : new URL(req.url, "http://localhost").pathname,
        ),
    );
    if (
      !path.startsWith(root + sep) ||
      path.includes(`${sep}.git${sep}`) ||
      (!path.startsWith(root + "/web/") &&
        !path.startsWith(root + "/examples/pdfs/") &&
        !path.startsWith(root + "/tests/skald-corpus/") &&
        !path.startsWith(root + "/node_modules/pdfjs-dist/build/") &&
        !path.startsWith(root + "/node_modules/pdfjs-dist/wasm/"))
    ) {
      res.writeHead(404).end();
      return;
    }
    const stat = statSync(path);
    if (!stat.isFile()) throw Error("not a file");
    res.writeHead(200, {
      "Content-Type": types[extname(path)] ?? "application/octet-stream",
      "Content-Length": stat.size,
      "Cache-Control": "no-cache",
      "X-Content-Type-Options": "nosniff",
      "Referrer-Policy": "no-referrer",
      "Content-Security-Policy":
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; connect-src 'self'; img-src 'self' blob: data:; style-src 'self' 'unsafe-inline'; font-src 'self'",
    });
    createReadStream(path).pipe(res);
  } catch {
    res.writeHead(404).end("Not found");
  }
}).listen(Number(process.env.PORT ?? 4173), "127.0.0.1", () =>
  console.log(
    "Skald View → http://127.0.0.1:" + Number(process.env.PORT ?? 4173),
  ),
);
