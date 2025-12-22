use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(&crate_dir).join("Sources/StateSetC/include");

    // Create the output directory if it doesn't exist
    std::fs::create_dir_all(&out_dir).ok();

    // Generate C header
    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(cbindgen::Language::C)
        .with_include_guard("STATESET_H")
        .with_documentation(true)
        .with_no_includes()
        .with_sys_include("stdint.h")
        .with_sys_include("stdbool.h")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_dir.join("stateset.h"));

    println!("cargo:rerun-if-changed=src/lib.rs");
}
