fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    // Use our own linker script: higher-half kernel layout expected by Limine.
    println!("cargo:rustc-link-arg-bins=-T{dir}/linker-x86_64.ld");
    println!("cargo:rerun-if-changed=linker-x86_64.ld");
}
