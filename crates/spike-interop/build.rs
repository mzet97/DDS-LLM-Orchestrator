//! Build script for spike-interop that forces static linking to pre-built CycloneDDS.

use std::env;
use std::path::PathBuf;

fn main() {
    let dds_enabled = env::var("CARGO_FEATURE_DDS").is_ok();
    if !dds_enabled {
        return;
    }

    // Try to find pre-built CycloneDDS library
    let lib_dir = if let Ok(build_dir) = env::var("CYCLONEDDS_BUILD") {
        PathBuf::from(build_dir).join("lib")
    } else {
        // Default location
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        manifest_dir.join(
            "../../../third_party/cyclonedds-rust/cyclonedds-rust/vendor/cyclonedds/build/lib",
        )
    };

    if lib_dir.exists() && lib_dir.join("libddsc.a").exists() {
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        // Link only ddsc (built without security/ssl, so no OpenSSL dependency)
        println!("cargo:rustc-link-lib=static:+whole-archive=ddsc");
        // System dependencies required by CycloneDDS
        println!("cargo:rustc-link-lib=dl");
        println!("cargo:rustc-link-lib=rt");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
        println!(
            "cargo:warning=Using pre-built CycloneDDS from {}",
            lib_dir.display()
        );
    } else {
        println!(
            "cargo:warning=CycloneDDS library not found at {}",
            lib_dir.display()
        );
        println!("cargo:warning=Please build CycloneDDS first: cd vendor/cyclonedds && mkdir -p build && cd build && cmake .. -DBUILD_SHARED_LIBS=OFF && cmake --build . --target ddsc");
    }
}
