fn main() {
    println!("cargo:rerun-if-changed=native/menu.m");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("native/menu.m")
            .flag("-fobjc-arc")
            .flag("-mmacosx-version-min=11.0")
            .compile("helper_menu");
        println!("cargo:rustc-link-lib=framework=AppKit");
    }
}
