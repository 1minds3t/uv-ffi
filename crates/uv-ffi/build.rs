fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:warning=╔═══════════════════════════════════════════════════════════════╗");
    println!("cargo:warning=║  uv-ffi — pre-built wheels available (use if build fails)     ║");
    println!("cargo:warning=╠═══════════════════════════════════════════════════════════════╣");
    println!("cargo:warning=║  PyPI   abi3 py3.8+  : x86_64, aarch64, macOS, Windows        ║");
    println!("cargo:warning=║  Extended            : musllinux, armv7/i686/ppc64le/s390x    ║");
    println!("cargo:warning=║                        riscv64, PyPy, GraalPy, 3.13t, cp37    ║");
    println!("cargo:warning=╚═══════════════════════════════════════════════════════════════╝");
    println!("cargo:warning=  pip install uv-ffi -f https://exotic-wheels.github.io/  # extended + cp37");
    println!("cargo:warning=  pip install uv-ffi --config-settings \"cargo-extra-args=--no-default-features\" # for cp37 from source");
}