// Modified for rhy-mac-ios on 2026-10-20; see NOTICE and docs/PROVENANCE.md.
// Mush8 - build.rs
// Constantin Clerc (c22dev) - 0.1
use std::fmt::Write;
use std::path::{Path, PathBuf};

const PAGE: u64 = 0x4000;
const FIRST_SEGMENT: u64 = 0x1_0400_0000;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(embedded_modules)");
    println!("cargo:rerun-if-env-changed=NX_MODULES_DIR");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("ios") {
        return;
    }
    build_launcher();
    println!("cargo:rustc-link-arg-bins=-Wl,-no_compact_unwind");
    let Ok(dir) = std::env::var("NX_MODULES_DIR") else {
        println!("cargo:warning=NX_MODULES_DIR is not set: the iOS build has no game code");
        return;
    };
    let dir = PathBuf::from(dir);
    let mut asm = String::new();
    let mut addr = FIRST_SEGMENT;
    let mut modules: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "layout"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned()).collect();
    modules.sort_by_key(|n| (if n == "main" {0} else if n == "sdk" {2} else {1}, n.clone()));
    let mut bindings = String::from("unsafe extern \"C\" {\n");
    let mut arms = String::new();
    for (i, name) in modules.iter().enumerate() {
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'), "unsafe module name");
        for part in ["x", "d", "t"] { writeln!(bindings, "static nx_mod_{name}_{part}: u8;").unwrap(); }
        writeln!(bindings, "static nx_mod_{name}_len: u64;").unwrap();
        writeln!(arms, "\"{name}\" => (&raw const nx_mod_{name}_x, &raw const nx_mod_{name}_d, &raw const nx_mod_{name}_t, nx_mod_{name}_len),").unwrap();
        let (bin, layout) = (dir.join(format!("{name}.bin")), dir.join(format!("{name}.layout")));
        println!("cargo:rerun-if-changed={}", bin.display());
        println!("cargo:rerun-if-changed={}", layout.display());
        let image = std::fs::read(&bin).unwrap_or_else(|e| panic!("{}: {e}", bin.display()));
        let (text, tramp) = read_layout(&layout);
        let x_end = text.next_multiple_of(PAGE);
        assert!(tramp % PAGE == 0 && tramp > x_end, "{name}: trampolines at {tramp:#x} are not on their own 16 KiB pages");
        let dynamic = nxfs::module::parse(&image).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        if let Some(r) = dynamic.relocs.iter().find(|r| r.offset < x_end || r.offset >= tramp) {
            panic!("{name}: relocation at {:#x} falls in code, which is read-only on iOS", r.offset);
        }
        let end = (image.len() as u64).next_multiple_of(PAGE);
        let bin = bin.canonicalize().unwrap();
        for (kind, lo, hi, prot) in [("X", 0, x_end, "rx"), ("D", x_end, tramp, "rw"), ("T", tramp, end, "rx")] {
            let seg = format!("__NX{i}_{kind}");
            let sym = format!("_nx_mod_{name}_{}", kind.to_lowercase());
            let len = hi.min(image.len() as u64).saturating_sub(lo);
            let flags = if prot == "rx" { "regular,pure_instructions" } else { "regular" };
            writeln!(asm, ".section {seg},__{},{flags}", if prot == "rx" { "text" } else { "data" }).unwrap();
            writeln!(asm, ".p2align 14\n.globl {sym}\n.no_dead_strip {sym}\n{sym}:").unwrap();
            if len > 0 {
                writeln!(asm, ".incbin \"{}\", {lo:#x}, {len:#x}", bin.display()).unwrap();
            }
            if hi - lo > len {
                writeln!(asm, ".space {:#x}", hi - lo - len).unwrap();
            }
            println!("cargo:rustc-link-arg-bins=-Wl,-segprot,{seg},{prot},{prot}");
            println!("cargo:rustc-link-arg-bins=-Wl,-segaddr,{seg},{addr:#x}");
            addr += hi - lo;
        }
        addr += PAGE;
        writeln!(asm, ".section __DATA,__const\n.p2align 3\n.globl _nx_mod_{name}_len\n_nx_mod_{name}_len:\n.quad {}", image.len()).unwrap();
    }
    bindings.push_str("}\nunsafe fn linked(name: &str) -> Option<(*const u8,*const u8,*const u8,u64)> { Some(unsafe { match name {\n");
    bindings.push_str(&arms);
    bindings.push_str("_ => return None, } }) }\n");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("module_bindings.rs"), bindings).unwrap();
    asm.push_str(".text\n");
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("modules.s");
    std::fs::write(&out, asm).unwrap();
    println!("cargo:rustc-cfg=embedded_modules");
}

fn build_launcher() {
    println!("cargo:rerun-if-changed=../../ios/Launcher.swift");
    let out = std::env::var("OUT_DIR").unwrap();
    let xcrun = |args: &[&str]| {
        let o = std::process::Command::new("xcrun").args(args).output().expect("xcrun");
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    };
    let sdk = xcrun(&["--sdk", "iphoneos", "--show-sdk-path"]);
    let swiftc = xcrun(&["--sdk", "iphoneos", "-f", "swiftc"]);
    let status = std::process::Command::new(&swiftc)
        .args(["-parse-as-library", "-emit-library", "-static", "-O", "-module-name", "RhythmLauncher"])
        .args(["-target", "arm64-apple-ios26.0", "-sdk", &sdk, "-module-cache-path", &format!("{out}/ModuleCache")])
        .arg("../../ios/Launcher.swift")
        .arg("-o")
        .arg(format!("{out}/librhythmlauncher.a"))
        .status()
        .expect("swiftc");
    assert!(status.success(), "swiftc failed");
    println!("cargo:rustc-link-search=native={out}");
    println!("cargo:rustc-link-lib=static=rhythmlauncher");
    println!("cargo:rustc-link-search=native={sdk}/usr/lib/swift");
    let toolchain = Path::new(&swiftc).parent().unwrap().parent().unwrap().join("lib/swift/iphoneos");
    println!("cargo:rustc-link-search=native={}", toolchain.display());
}

fn read_layout(path: &Path) -> (u64, u64) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let get = |key: &str| {
        let line = text.lines().find(|l| l.split_whitespace().next() == Some(key)).unwrap_or_else(|| panic!("{}: no {key}", path.display()));
        let f: Vec<&str> = line.split_whitespace().collect();
        let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x"), 16).unwrap();
        (hex(f[1]), hex(f[2]))
    };
    (get("text").1, get("tramp").0)
}
