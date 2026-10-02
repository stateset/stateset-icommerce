# Hand-written declaration fragments

Every `*.d.ts` in this directory is appended to the napi-generated `index.d.ts`
by `scripts/postbuild-types.mjs` (run from `scripts/postbuild.mjs` after every
`npm run build*`). Use them for:

- literal unions referenced from Rust via `#[napi(ts_type = "OrderStatus")]`;
- interfaces for `serde_json::Value` payloads referenced via `ts_type`,
  `ts_args_type` or `ts_return_type`;
- the JavaScript-side surface `index.js` layers over native classes.

Fragments are concatenated in filename order after `index-augment.d.ts`.
Names must be unique across fragments. Keep each fragment self-contained and
commented: the generated file is what consumers read.
