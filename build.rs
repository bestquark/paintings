fn main() {
    println!("cargo:rerun-if-changed=adapters/wallpaper.swift");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let arch = if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        let status = std::process::Command::new("swiftc")
            .args([
                "-O",
                "-target",
                &format!("{arch}-apple-macos13.0"),
                "adapters/wallpaper.swift",
                "-o",
            ])
            .arg(std::path::Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("wallpaper"))
            .status()
            .expect("Building macOS support requires Apple's command-line tools (swiftc)");
        assert!(status.success(), "Could not build the macOS desktop helper");
    }
}
