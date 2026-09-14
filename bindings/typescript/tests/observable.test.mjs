import assert from "node:assert/strict";
import { test } from "node:test";
import { acceptedAt, cursored, echoGuard, observe } from "../src/observable.ts";

const flush = () => new Promise(resolve => queueMicrotask(resolve));

/** A change stream a test drives by hand, in the shape the core's streams take. */
function revisions() {
  let waiting = null;
  let closed = false;
  return {
    disposals: 0,
    emit(revision) {
      const resolve = waiting;
      waiting = null;
      resolve?.({ value: revision, done: false });
    },
    changes() {
      const source = this;
      return {
        dispose() {
          source.disposals++;
          closed = true;
          const resolve = waiting;
          waiting = null;
          resolve?.({ value: undefined, done: true });
        },
        [Symbol.asyncIterator]: () => ({
          next: () =>
            closed
              ? Promise.resolve({ value: undefined, done: true })
              : new Promise(resolve => {
                  waiting = resolve;
                }),
        }),
      };
    },
  };
}

test("a burst refreshes once and dispose stops the stream", async () => {
  const source = revisions();
  let value = 0;
  let reads = 0;
  const store = observe(
    () => {
      reads++;
      return value;
    },
    () => source.changes(),
  );
  let renders = 0;
  store.subscribe(() => renders++);
  for (let i = 1; i <= 10000; i++) {
    value = i;
    source.emit(BigInt(i));
  }
  await flush();
  assert.equal(store.getSnapshot(), 10000);
  assert.equal(reads, 2);
  assert.equal(renders, 1);

  source.emit(10001n);
  store.dispose();
  store.dispose();
  await flush();
  assert.equal(reads, 2);
  assert.equal(source.disposals, 1);
});

test("unsubscribing stops one listener and leaves the others", async () => {
  const source = revisions();
  let value = 0;
  const store = observe(() => value, () => source.changes());
  let kept = 0;
  const stop = store.subscribe(() => assert.fail("a dropped listener was called"));
  store.subscribe(() => kept++);
  stop();
  value = 1;
  source.emit(1n);
  await flush();
  assert.equal(store.getSnapshot(), 1);
  assert.equal(kept, 1);
  store.dispose();
});

test("a stream that ends leaves the last snapshot readable", async () => {
  const source = revisions();
  let value = 0;
  const store = observe(() => value, () => source.changes());
  value = 7;
  source.emit(1n);
  await flush();
  source.changes().dispose();
  value = 9;
  await flush();
  assert.equal(store.getSnapshot(), 7);
  store.dispose();
});

test("the cursor appends by id and starts over when the source rewinds", () => {
  const rows = [{ id: 1n }, { id: 2n }, { id: 3n }];
  const reads = [];
  const read = cursored(
    () => rows.slice(),
    afterId => {
      reads.push(afterId);
      return rows.filter(row => row.id > afterId);
    },
    row => row.id,
  );
  assert.deepEqual(read().map(row => row.id), [1n, 2n, 3n]);
  // The first read starts over, so it asks the source for nothing.
  assert.deepEqual(reads, []);
  rows.push({ id: 4n });
  assert.deepEqual(read().map(row => row.id), [1n, 2n, 3n, 4n]);
  // One row of overlap is the whole rewind test: it asked from 2, not from 0.
  assert.deepEqual(reads, [2n]);
  // A source that no longer holds the row the cursor stands on has rebound.
  rows.length = 0;
  rows.push({ id: 1n });
  assert.deepEqual(read().map(row => row.id), [1n]);
});

test("the echo guard marks our own writes and nests", () => {
  const guard = echoGuard();
  assert.equal(guard.isApplying, false);
  const seen = guard.apply(() => guard.apply(() => guard.isApplying));
  assert.equal(seen, true);
  assert.equal(guard.isApplying, false);
});

test("an accepted stamp is a date only inside the one bounds rule", () => {
  assert.equal(acceptedAt(0n), null);
  assert.equal(acceptedAt(253402300800000n), null);
  assert.equal(acceptedAt(1700000000000n)?.toISOString(), new Date(1700000000000).toISOString());
});
