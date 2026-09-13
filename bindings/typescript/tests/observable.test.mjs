import assert from "node:assert/strict";
import { test } from "node:test";
import { ObservableState } from "../src/observable.ts";

const flush = () => new Promise(resolve => queueMicrotask(resolve));

test("subscribe precedes the initial and replacement snapshots", () => {
  let value = 0;
  const observer = new ObservableState(() => value, () => {
    value = 1;
    return { cancel() {} };
  });
  assert.equal(observer.getSnapshot(), 1);
  observer.observe(() => value, () => {
    value = 2;
    return { cancel() {} };
  });
  assert.equal(observer.getSnapshot(), 2);
  observer.dispose();
});

test("a burst refreshes once and dispose suppresses queued work", async () => {
  let notify;
  let value = 0;
  let reads = 0;
  let cancellations = 0;
  const observer = new ObservableState(() => { reads++; return value; }, callback => {
    notify = callback;
    return { cancel() { cancellations++; } };
  });
  let renders = 0;
  observer.subscribe(() => renders++);
  for (let i = 1; i <= 10000; i++) { value = i; notify(); }
  await flush();
  assert.equal(observer.getSnapshot(), 10000);
  assert.equal(reads, 2);
  assert.equal(renders, 1);
  notify();
  observer.dispose();
  observer.dispose();
  await flush();
  assert.equal(reads, 2);
  assert.equal(cancellations, 1);
});

test("pending work reads the replacement source", async () => {
  let notify;
  const observer = new ObservableState(() => "old", callback => {
    notify = callback;
    return { cancel() {} };
  });
  notify();
  observer.observe(() => "new", () => ({ cancel() {} }));
  await flush();
  assert.equal(observer.getSnapshot(), "new");
  observer.dispose();
});

test("replacement rejects stale callbacks and queued invalidations", async () => {
  let stale;
  let current;
  let reads = 0;
  const observer = new ObservableState(() => "old", callback => {
    stale = callback;
    return { cancel() {} };
  });
  stale();
  observer.observe(() => { reads++; return "new"; }, callback => {
    current = callback;
    return { cancel() {} };
  });
  await flush();
  stale();
  await flush();
  assert.equal(reads, 1);
  current();
  stale();
  await flush();
  assert.equal(reads, 2);
  observer.dispose();
});
