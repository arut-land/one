import * as ffi from "@arut/ffi";
import { uuidV7 } from "./ids";
export async function createSession(scope: string): Promise<ffi.ProductSessionHandle> {
  await (ffi as typeof ffi & { initialized: Promise<void> }).initialized;
  return ffi.createBrowserSession(scope, { newId: uuidV7 });
}
