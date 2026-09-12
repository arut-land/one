# Browser runtime

The page supplies UUIDv7 time and entropy through a generated BoltFFI callback.
The wasm core has no wasm-bindgen imports. Futures and invalidations use BoltFFI
host polling on the page's event loop; no threads are required.
