import type { ObservableStore } from "@arut/bindings-typescript";
import { useSyncExternalStore } from "react";

export function useObservable<T>(store: ObservableStore<T>): T {
  return useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);
}
