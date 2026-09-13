import * as ffi from "@arut/ffi";
export async function createSession(scope: string, ids: ffi.HostIds): Promise<ffi.ProductSessionHandle> {
  await (ffi as typeof ffi & { initialized: Promise<void> }).initialized;
  return ffi.createBrowserSession(scope, ids);
}
