use std::{env, path::PathBuf, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for source in ["overlay", "dock_badges", "status_item", "focus_tracking"] {
        let input = format!("src/{source}.m");
        let output = output_dir.join(format!("{source}.o"));
        let status = Command::new("clang")
            .args(["-fobjc-arc", "-c", &input, "-o"])
            .arg(&output)
            .status()
            .unwrap_or_else(|_| panic!("failed to run clang for {input}"));
        assert!(status.success(), "failed to compile {input}");
        println!("cargo:rustc-link-arg-bins={}", output.display());
        println!("cargo:rerun-if-changed={input}");
    }
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-lib=objc");
}
