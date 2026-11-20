fn main() {
    tauri_build::build();

    // Link the generated Windows resource (Common Controls v6 manifest) into test binaries
    // so `cargo test` does not fail with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139) on TaskDialogIndirect.
    #[cfg(windows)]
    {
        if let Ok(out_dir) = std::env::var("OUT_DIR") {
            let resource_lib = std::path::Path::new(&out_dir).join("libresource.a");
            if resource_lib.exists() {
                println!("cargo:rustc-link-arg-tests={}", resource_lib.display());
            }
        }
    }
}
