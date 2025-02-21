fn main() {
    // Nitro Enclaves are only supported on linux, so we wont actually be using
    // the c-sdk if we are on macos.
    // #[cfg(not(target_os = "macos"))]
    let dst = cmake::build("aws-nitro-enclaves-sdk-c");

    println!("cargo:rustc-link-search=native={}/lib64", dst.display());
    println!("cargo:rustc-link-lib=static=aws-c-common");
    println!("cargo:rustc-link-lib=static=aws-c-auth");
    println!("cargo:rustc-link-lib=static=aws-c-http");
    println!("cargo:rustc-link-lib=static=aws-c-io");
    println!("cargo:rustc-link-lib=static=aws-nitro-enclaves-sdk-c");
}