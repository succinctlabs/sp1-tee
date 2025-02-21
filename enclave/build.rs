fn main() {
    // Nitro Enclaves are only supported on linux, so we wont actually be using
    // the c-sdk if we are on macos.
    // #[cfg(not(target_os = "macos"))]
    cmake::Config::new("aws-nitro-enclaves-sdk-c/").build();

    println!("cargo:rustc-link-lib=static=aws-nitro-enclaves-sdk-c");
}