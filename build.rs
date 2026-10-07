use std::{env, path::PathBuf, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("overlay.o");
    let status = Command::new("clang")
        .args(["-fobjc-arc", "-c", "src/overlay.m", "-o"])
        .arg(&output)
        .status()
        .expect("failed to run clang for the AppKit overlay");
    assert!(status.success(), "failed to compile the AppKit overlay");
    println!("cargo:rustc-link-arg-bins={}", output.display());
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-lib=objc");
    println!("cargo:rerun-if-changed=src/overlay.m");
}
