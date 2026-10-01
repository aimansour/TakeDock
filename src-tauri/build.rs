fn main() {
    #[cfg(feature = "desktop")]
    tauri_build::build();
    // Cargo examples do not receive the binary-only Tauri Windows resource.
    // The unbundled updater verifier also links dialogs and requires Common Controls v6.
    if cfg!(feature = "desktop") && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    {
        println!("cargo:rustc-link-arg-examples=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-examples=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
