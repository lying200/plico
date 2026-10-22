use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=assets/branding/plico.ico");
    println!("cargo:rerun-if-changed=assets/branding/plico.rc");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var_os("CARGO_FEATURE_DESKTOP").is_none()
    {
        return;
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("plico.res");
    let status = Command::new(env::var_os("LLVM_RC").unwrap_or_else(|| "llvm-rc".into()))
        .args(["/no-preprocess", "/FO"])
        .arg(&output)
        .arg("assets/branding/plico.rc")
        .status()
        .expect("Windows icon embedding requires llvm-rc (provided by devenv)");
    assert!(status.success(), "Compile Plico Windows icon resource");
    println!("cargo:rustc-link-arg-bin=plico={}", output.display());
}
