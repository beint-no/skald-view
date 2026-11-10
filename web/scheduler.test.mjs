import { test } from "node:test";
import assert from "node:assert/strict";
import { LatestQueue } from "./scheduler.mjs";
test("bounds pending work and replaces obsolete renders", async () => {
  let complete;
  const started = [];
  const q = new LatestQueue((v) => {
    started.push(v);
    return new Promise((r) => (complete = r));
  });
  const first = q.submit(1);
  const obsolete = q.submit(2);
  const rejected = assert.rejects(obsolete, { name: "AbortError" });
  const latest = q.submit(3);
  assert.deepEqual(started, [1]);
  complete("first");
  assert.equal(await first, "first");
  await rejected;
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(started, [1, 3]);
  complete("last");
  assert.equal(await latest, "last");
});
test("continues after failure", async () => {
  const q = new LatestQueue(async (v) => {
    if (v === 1) throw Error("failed");
    return v;
  });
  await assert.rejects(q.submit(1));
  assert.equal(await q.submit(2), 2);
});
test("closing rejects pending and new requests", async () => {
  let complete;
  const q = new LatestQueue(() => new Promise((r) => (complete = r)));
  const first = q.submit(1);
  const queued = q.submit(2);
  const rejected = assert.rejects(queued, { name: "AbortError" });
  q.close();
  await rejected;
  await assert.rejects(q.submit(3));
  complete(1);
  assert.equal(await first, 1);
});
