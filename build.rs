use std::env;
use std::path::PathBuf;
use std::process::Command;

fn run(command: &mut Command, description: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to start {description}: {error}"));
    assert!(status.success(), "{description} failed with {status}");
}

fn main() {
    println!("cargo:rerun-if-changed=linker.ld");
    println!("cargo:rerun-if-changed=native/push_policy.f90");

    if env::var_os("CARGO_FEATURE_FORTRAN_POLICY").is_some() {
        let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
        let object = output.join("push_policy.o");
        let archive = output.join("libpush_policy.a");
        let compiler = env::var_os("FC").unwrap_or_else(|| "gfortran".into());
        run(
            Command::new(compiler)
                .arg("-c")
                .arg("-O2")
                .arg("-fPIC")
                .arg(format!("-J{}", output.display()))
                .arg("native/push_policy.f90")
                .arg("-o")
                .arg(&object),
            "Fortran supervision policy compilation",
        );
        run(
            Command::new("ar").arg("crs").arg(&archive).arg(&object),
            "Fortran supervision policy archive",
        );
        println!("cargo:rustc-link-search=native={}", output.display());
        println!("cargo:rustc-link-lib=static=push_policy");
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("none") {
        return;
    }

    let linker_script = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by Cargo"),
    )
    .join("linker.ld");
    println!(
        "cargo:rustc-link-arg-bin=push=-T{}",
        linker_script.display()
    );
    println!("cargo:rustc-link-arg-bin=push=--no-pie");
    println!("cargo:rustc-link-arg-bin=push=--no-dynamic-linker");
    println!("cargo:rustc-link-arg-bin=push=--gc-sections");
}
