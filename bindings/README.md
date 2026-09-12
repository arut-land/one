# bindings

`ffi` is the minimal BoltFFI composition and export layer over feature-owned product clients. The language packages publish generated state through native observable types; surfaces never import generated FFI packages directly.

The `qt` crate is the same-process adapter. It accepts product clients from the executable composition root, publishes snapshots as Rust-backed Qt objects, properties, primitives, and enums, and coalesces revision bursts onto the QObject thread. It does not select a runtime or expose generated BoltFFI bindings.

Generate platform packages from the BoltFFI layer:

```sh
cd bindings/ffi
boltffi pack apple
boltffi pack android
boltffi pack csharp
boltffi pack wasm
```

Generated packages are written to `bindings/generated/`, consumed only by wrappers in this directory, and are not committed.
