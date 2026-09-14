import { type Changes, type EchoGuard, echoGuard, observe, type ObservableStore } from "./observable";
import { useEffect, useState, useSyncExternalStore } from "react";

export function useObservable<T>(store: ObservableStore<T>): T {
  return useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);
}

/**
 * One handle's projection, re-read whenever that handle reports a change.
 *
 * `source` identifies the handle: a new one builds a new store and the old one
 * is disposed, so nothing here ever shows a fabricated state while a handle is
 * being replaced -- every render reads a real projection of a real handle.
 */
export function useHandle<T>(source: object, read: () => T, changes: () => Changes): T {
  return useObservable(useOwned(source, () => observe(read, changes)));
}

/** A handle held for as long as `key` stands, and disposed when it does not. */
export function useOwned<T extends { dispose(): void }>(key: unknown, create: () => T): T {
  const [held, setHeld] = useState(() => ({ key, value: create() }));
  let current = held;
  if (!Object.is(current.key, key)) {
    current.value.dispose();
    current = { key, value: create() };
    setHeld(current);
  }
  const value = current.value;
  useEffect(() => () => value.dispose(), [value]);
  return value;
}

/**
 * The echo rule, held across renders.
 *
 * The guard is a ref, not state: marking a write as ours must not schedule a
 * render of its own.
 */
export function useEcho(): EchoGuard {
  const [guard] = useState(echoGuard);
  return guard;
}
