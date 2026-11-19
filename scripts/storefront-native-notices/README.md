# Recreate the pilot native notice bundle

Use Python 3.11 or later, Cargo and the Rust 1.90.0 x86-64 Linux toolchain. From the repository root, fetch the locked public sources if they are absent from the local Cargo cache, then supply the native tarball in the pilot project's vendor directory.

```sh
cargo fetch --locked --target x86_64-unknown-linux-gnu
rustup toolchain install 1.90.0 --profile minimal
python3.11 scripts/storefront-native-notices/collect.py \
  /path/to/project/vendor/stateset/stateset-embedded-linux-x64-gnu-1.40.1.tgz \
  /path/to/new/native-notices
```

The collection itself runs offline. It verifies the tarball and native member against the original build, each crates.io source against the repository lockfile, the five restored napi-rs licenses against the pinned upstream bytes, and the standard-library copyright document against the original toolchain. For a separate crate cache or toolchain documentation directory, pass `--cache` or `--rust-doc`. The Rust compiler component includes the copyright document under `share/doc/rust/`. A successful run writes an inventory, upstream notices, source headers and public source links into a new directory. Package that directory alongside StateSet's license texts.

The checked-in `pilot-build.json` records the features and exact registry package versions reachable from the original release Cargo fingerprint. To audit the capture with the retained **original native build directory**, compare the following output with the checked-in manifest. Both archive and ELF member must match. A new build needs its own audited capture.

```sh
python3.11 scripts/storefront-native-notices/capture.py \
  /path/to/original/target/release-node \
  /path/to/project/vendor/stateset/stateset-embedded-linux-x64-gnu-1.40.1.tgz \
  /path/to/toolchain/share/doc/rust/COPYRIGHT-library.html \
  | diff - scripts/storefront-native-notices/pilot-build.json
```
