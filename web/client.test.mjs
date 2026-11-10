import { test } from "node:test";
import assert from "node:assert/strict";
import { SkaldDocument } from "./client.mjs";
class SilentWorker {
  terminated = false;
  postMessage() {}
  terminate() {
    this.terminated = true;
  }
}
test("deadline terminates nonresponding worker and rejects the request", async () => {
  globalThis.Worker = SilentWorker;
  const doc = new SkaldDocument({ timeoutMs: 5 });
  await assert.rejects(doc.open(new ArrayBuffer(0)), /deadline exceeded/);
  assert.equal(doc.worker.terminated, true);
  assert.equal(doc.pending.size, 0);
  await assert.rejects(doc.render(0, 1), /closed/i);
});
test("close rejects outstanding requests and releases worker", async () => {
  globalThis.Worker = SilentWorker;
  const doc = new SkaldDocument();
  const p = doc.open(new ArrayBuffer(0));
  const rejected = assert.rejects(p, { name: "AbortError" });
  doc.close();
  await rejected;
  assert.equal(doc.worker.terminated, true);
  assert.equal(doc.pending.size, 0);
});
