# Linux storefront preview native dependency notices

The shipped Node addon contains the StateSet embedded engine. This directory accompanies the Linux x86-64 GNU 1.40.1 preview. The project supplies StateSet's license alongside this directory.

Open `inventory.json` to find each dependency's version, enabled features, checksum, upstream source archive, license expression and notice files. Under `crates/`, each package directory retains the upstream paths to dedicated notices. `SOURCE-HEADERS.txt` groups leading source notices under the files where they appear. Five napi-rs crate archives omitted their shared license. Their entries record the upstream license URL at each package's recorded source revision. `rust-standard-library/COPYRIGHT-library.html` contains the Rust project's attribution for its precompiled standard library.

The inventory follows Cargo's retained fingerprints and dependency records for the packaged binary. It includes build scripts, procedural macros and their dependencies, and preserves nested notices from the selected crate archives. The fingerprint graph includes 279 compilation units, 214 third-party crate versions and eight StateSet crates. The native member's SHA-256 is `34bf601bd5944c3240b8e9c89d36a2885637c17b0af8eeff56acffebcdf23adf`. The build used Rust 1.90.0 and the `release-node` profile. The original build included working-tree changes.

The inventory covers this native artifact. JavaScript package sources, hashes and declared licenses are in the storefront's npm lockfile. An evaluator's operating system and container image supply their own libraries and notices.
