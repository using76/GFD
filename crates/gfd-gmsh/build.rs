//! Links the Gmsh SDK only when the `gmsh` feature is enabled. Without it the
//! crate is a pure-Rust stub and this script is a no-op (so the workspace builds
//! with no Gmsh SDK / C++ toolchain).
//!
//! Requires `GMSH_LIB_DIR` to point at the Gmsh SDK `lib` folder (containing
//! `gmsh.dll.lib` + `gmsh-*.dll` on Windows, or `libgmsh.so`/`.dylib` elsewhere).

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=GMSH_LIB_DIR");
    println!("cargo:rerun-if-changed=build.rs");

    if env::var_os("CARGO_FEATURE_GMSH").is_none() {
        return; // feature off → pure-Rust stub, nothing to link
    }

    let lib_dir = PathBuf::from(env::var("GMSH_LIB_DIR").expect(
        "GMSH_LIB_DIR must point to the Gmsh SDK lib folder when building with --features gmsh",
    ));

    // On Windows/MSVC the import lib is named `gmsh.dll.lib`; the linker wants
    // `gmsh.lib`. Copy it into OUT_DIR under the expected name and search there.
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let win_import = lib_dir.join("gmsh.dll.lib");
    if win_import.exists() {
        std::fs::copy(&win_import, out.join("gmsh.lib")).expect("copy gmsh import lib to OUT_DIR");
        println!("cargo:rustc-link-search=native={}", out.display());
    }
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=gmsh");
}
