use std::path::PathBuf;

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = PathBuf::from(&crate_dir).join("Sources/StateSetC/include");

    std::fs::create_dir_all(&out_dir).expect("create Swift C include directory");
    std::fs::write(
        out_dir.join("stateset.h"),
        r#"#ifndef STATESET_H
#define STATESET_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void *StateSetHandle;

StateSetHandle stateset_commerce_new(const char *db_path);
void stateset_commerce_free(StateSetHandle handle);
void stateset_string_free(char *s);
char *stateset_get_last_error(void);

#ifdef __cplusplus
}
#endif

#endif
"#,
    )
    .expect("write Swift C header");

    println!("cargo:rerun-if-changed=src/lib.rs");
}
