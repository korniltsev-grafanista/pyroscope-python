fn main() {
    println!("cargo::rustc-check-cfg=cfg(use_libunwind)");
}
