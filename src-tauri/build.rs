fn main() {
    tauri_build::build();

    // Tauri links its Windows manifest to app binaries by default. The mock
    // runtime tests also need Common Controls v6 (e.g. TaskDialogIndirect).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let resource = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            "resource.lib"
        } else {
            "libresource.a"
        };
        let path = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join(resource);
        println!("cargo:rustc-link-arg-tests={}", path.display());
    }
}
