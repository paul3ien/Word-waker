fn main() {
    cc::Build::new()
        .file("src/bridge/coreml_bridge.mm")
        .flag("-fobjc-arc")
        .flag("-std=c++20")
        .flag("-mmacosx-version-min=14.0")
        .compile("coreml_bridge");

    println!("cargo:rustc-link-lib=framework=CoreML");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}
