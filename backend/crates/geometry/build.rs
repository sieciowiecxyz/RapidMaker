fn main() {
    println!("cargo:rerun-if-env-changed=OCCT_INCLUDE_DIR");
    println!("cargo:rerun-if-env-changed=OCCT_LIB_DIR");
    let include =
        std::env::var("OCCT_INCLUDE_DIR").unwrap_or_else(|_| "/usr/include/opencascade".into());
    if let Ok(path) = std::env::var("OCCT_LIB_DIR") {
        println!("cargo:rustc-link-search=native={path}");
    }
    cxx_build::bridge("src/step.rs")
        .file("src/step.cpp")
        .include("include")
        .include(include)
        .std("c++17")
        .compile("rapidmaker-step");
    for library in [
        "TKDESTEP",
        "TKXSBase",
        "TKMesh",
        "TKTopAlgo",
        "TKBRep",
        "TKMath",
        "TKernel",
    ] {
        println!("cargo:rustc-link-lib={library}");
    }
    for source in ["src/step.rs", "src/step.cpp", "include/step.hxx"] {
        println!("cargo:rerun-if-changed={source}");
    }
}
