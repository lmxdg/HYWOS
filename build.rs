use std::env;
use std::path::PathBuf;
use std::process::Command;

fn exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn pick(names: &[&str]) -> Option<String> {
    for n in names {
        if exists(n) {
            return Some((*n).to_string());
        }
    }
    None
}

fn run(cmd: &mut Command) {
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("无法启动 {:?}: {}", cmd, e));
    if !status.success() {
        panic!("命令执行失败: {:?}", cmd);
    }
}

/// 返回 (编译器, 是否 clang)。UEFI 目标固定 x86_64，构建机可能是 aarch64。
fn select_c(names: &[&str], env_key: &str, fallback: &str, clang_fallback: &str) -> (String, bool) {
    if let Ok(v) = env::var(env_key) {
        let is_clang = v.contains("clang");
        return (v, is_clang);
    }
    if let Some(v) = pick(names) {
        return (v, false);
    }
    if let Some(v) = pick(&[clang_fallback]) {
        return (v, true);
    }
    (fallback.to_string(), false)
}

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let kdir = manifest.join("kernel");

    let (cc, cc_clang) = select_c(
        &[
            "x86_64-linux-gnu-gcc",
            "x86_64-openEuler-linux-gnu-gcc",
            "x86_64-unknown-linux-gnu-gcc",
        ],
        "HYW_CC",
        "gcc",
        "clang",
    );
    let (cxx, cxx_clang) = select_c(
        &[
            "x86_64-linux-gnu-g++",
            "x86_64-openEuler-linux-gnu-g++",
            "x86_64-unknown-linux-gnu-g++",
        ],
        "HYW_CXX",
        "g++",
        "clang++",
    );
    

    println!("cargo:rerun-if-changed=kernel/include/hyw_kernel.h");
    println!("cargo:rerun-if-changed=kernel/core/hybrid_kernel.c");
    println!("cargo:rerun-if-changed=kernel/core/driver_bus.cpp");
    println!("cargo:rerun-if-env-changed=HYW_CC");
    println!("cargo:rerun-if-env-changed=HYW_CXX");
    

    let files = [
        ("core/hybrid_kernel.c", false, cc_clang),
        ("core/driver_bus.cpp", true, cxx_clang),
    ];

    let mut objs: Vec<PathBuf> = Vec::new();
    for (rel, is_cpp, is_clang) in files.iter() {
        let src = kdir.join(rel);
        let stem = rel.replace('/', "_").replace(".c", "").replace(".cpp", "");
        let obj = out.join(format!("{}.o", stem));
        let mut cmd = Command::new(if *is_cpp { &cxx } else { &cc });
        if *is_clang {
            cmd.arg("--target=x86_64-pc-windows-msvc");
        }
        if *is_cpp {
            cmd.arg("-nostdinc++")
                .arg("-fno-exceptions")
                .arg("-fno-rtti")
                .arg("-fno-threadsafe-statics")
                .arg("-fno-use-cxa-atexit");
        }
        cmd.arg("-c")
            .arg("-O2")
            .arg("-ffreestanding")
            .arg("-fno-stack-protector")
            .arg("-fno-builtin")
            .arg("-fno-asynchronous-unwind-tables")
            .arg("-fno-unwind-tables")
            .arg("-fno-pic")
            .arg("-mno-red-zone")
            .arg("-fno-omit-frame-pointer")
            .arg("-Wall")
            .arg("-Wextra")
            .arg("-I")
            .arg(kdir.join("include"))
            .arg("-o")
            .arg(&obj)
            .arg(&src);
        run(&mut cmd);
        objs.push(obj);
    }

    for o in objs.iter() {
        println!("cargo:rustc-link-arg={}", o.display());
    }
}