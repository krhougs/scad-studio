# @budn/app-server-protocol

`@budn/app-server-protocol` re-exports the wasm-bindgen output for the Rust protocol codec and validation helpers.

TypeScript callers pass typed request parameters to command-specific wasm helpers. This package does not implement Borsh, path validation, relative path resolution, or full envelope serialization logic in TypeScript.

Thrown protocol errors are structured JS objects:

```ts
type ProtocolWasmError = {
  code: string;
  message: string;
};
```
