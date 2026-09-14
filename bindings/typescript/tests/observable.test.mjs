import assert from "node:assert/strict";
import { test } from "node:test";
import { chatReader, observe } from "../src/observable.ts";

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

/** A chat handle over a fixed transcript, as the wasm and bridge ports both read. */
function transcript(messages) {
  return {
    state: () => ({ id: "c", lastMessageId: messages.at(-1)?.id ?? 0n, status: 0, error: null, canSend: true, isSending: false, isEmpty: messages.length === 0 }),
    messagesAfter: afterId => messages.filter(message => message.id > afterId),
  };
}

test("the transcript appends new rows and starts over when the handle rebinds", () => {
  const rows = [{ id: 1n, text: "one" }, { id: 2n, text: "two" }];
  const chat = transcript(rows);
  const read = chatReader(chat);
  assert.deepEqual(read().messages.map(message => message.text), ["one", "two"]);
  rows.push({ id: 3n, text: "three" });
  assert.deepEqual(read().messages.map(message => message.text), ["one", "two", "three"]);
  // The editor bridge rebinds one handle to another conversation: a transcript
  // that moved backwards is not one this cache has a prefix of.
  rows.length = 0;
  rows.push({ id: 1n, text: "elsewhere" });
  assert.deepEqual(read().messages.map(message => message.text), ["elsewhere"]);
});
