fn main() {
    // Nitro Enclaves are only supported on linux, so we wont actually be using
    // the c-sdk if we are on macos.
    // #[cfg(not(target_os = "macos"))]
    let dst = cmake::build("aws-nitro-enclaves-sdk-c");

    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-lib=static=aws-nitro-enclaves-sdk-c");
}