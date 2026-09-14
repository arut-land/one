import assert from "node:assert/strict";
import { test } from "node:test";
import { projectionHost, projectionPort } from "../src/bridge.ts";

/** A macrotask: every microtask the host queued has run by the time it resolves. */
const flush = () => new Promise(resolve => setTimeout(resolve, 0));

/** A change stream a test drives by hand, in the shape the core's streams take. */
function revisions() {
  let waiting = null;
  return {
    emit(revision) {
      const resolve = waiting;
      waiting = null;
      resolve?.({ value: revision, done: false });
    },
    changes: () => ({
      [Symbol.asyncIterator]: () => ({
        next: () =>
          new Promise(resolve => {
            waiting = resolve;
          }),
      }),
    }),
  };
}

/** A host and a view wired to each other, as an editor host and its webview are. */
function wired(published) {
  const intents = [];
  let port;
  const host = projectionHost(published, message => port?.receive(message.values));
  port = projectionPort(host.snapshot(), message => {
    if (message.type === "intent") intents.push(message);
  });
  return { host, port, intents };
}

test("a projection crosses whole, with its identifiers intact", async () => {
  const source = revisions();
  let rows = [{ id: 1n, text: "one" }];
  const { host, port } = wired([{ name: "rows", read: () => rows, changes: source.changes }]);

  assert.deepEqual(port.state("rows"), [{ id: 1n, text: "one" }]);
  assert.equal(typeof host.snapshot().rows[0].id, "object");

  const changes = port.changes("rows")[Symbol.asyncIterator]();
  const next = changes.next();
  rows = [...rows, { id: 2n, text: "two" }];
  source.emit(1n);
  await flush();
  assert.equal((await next).done, false);
  assert.deepEqual(port.state("rows"), [
    { id: 1n, text: "one" },
    { id: 2n, text: "two" },
  ]);
  host.dispose();
});

test("a value of our own stands until the host carries it back", async () => {
  const source = revisions();
  let draft = "";
  const { host, port, intents } = wired([{ name: "draft", read: () => ({ text: draft }), changes: source.changes }]);

  port.echo("draft", { text: "typed" });
  port.intent("draft", { text: "typed" });
  assert.deepEqual(port.state("draft"), { text: "typed" });
  assert.deepEqual(intents, [{ type: "intent", name: "draft", payload: { text: "typed" } }]);

  // An older projection does not put the field back the way it was.
  source.emit(1n);
  await flush();
  assert.deepEqual(port.state("draft"), { text: "typed" });

  draft = "typed";
  source.emit(2n);
  await flush();
  assert.deepEqual(port.state("draft"), { text: "typed" });

  // Once the host has it, what the host says goes again.
  draft = "elsewhere";
  source.emit(3n);
  await flush();
  assert.deepEqual(port.state("draft"), { text: "elsewhere" });
  host.dispose();
});

test("only the projections that changed tick", async () => {
  const rowSource = revisions();
  const listSource = revisions();
  let rows = [];
  const { host, port } = wired([
    { name: "rows", read: () => rows, changes: rowSource.changes },
    { name: "list", read: () => ["a"], changes: listSource.changes },
  ]);
  let ticks = 0;
  void (async () => {
    for await (const _ of port.changes("list")) ticks++;
  })();
  rows = [{ id: 7n }];
  rowSource.emit(1n);
  await flush();
  assert.deepEqual(port.state("rows"), [{ id: 7n }]);
  assert.equal(ticks, 0);
  host.dispose();
});

test("one reader stands for several names, and an echo releases on its own entry", async () => {
  const source = revisions();
  let draft = "";
  let revision = 0n;
  const { host, port } = wired([
    { name: "draft", read: () => draft, changes: source.changes },
    { name: "composer", read: () => ({ revision }), changes: source.changes },
  ]);

  let ticks = 0;
  void (async () => {
    for await (const _ of port.changes("composer", "draft")) ticks++;
  })();

  // What the person typed stands, and the state around it still arrives.
  port.echo("draft", "typed");
  assert.equal(port.state("draft"), "typed");
  revision = 1n;
  source.emit(1n);
  await flush();
  assert.equal(port.state("draft"), "typed");
  assert.deepEqual(port.state("composer"), { revision: 1n });

  draft = "typed";
  revision = 2n;
  source.emit(2n);
  await flush();
  draft = "elsewhere";
  revision = 3n;
  source.emit(3n);
  await flush();
  assert.equal(port.state("draft"), "elsewhere");
  // Every name that changed ticked: the echo, the three revisions, and the
  // draft the host finally moved. A reader coalesces them into one read.
  assert.equal(ticks, 5);
  host.dispose();
});
