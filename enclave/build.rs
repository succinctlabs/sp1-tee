fn main() {
    // Nitro Enclaves are only supported on linux, so we wont actually be using
    // the c-sdk if we are on macos.
    // #[cfg(not(target_os = "macos"))]
    let dst = cmake::Config::new("aws-nitro-enclaves-sdk-c").define("BUILD_DEPS", "ON").build();

    println!("cargo:rustc-link-search=native={}/lib64", dst.display());

    println!("cargo:rustc-link-lib=static=aws-nitro-enclaves-sdk-c");

    // aws-c-auth first, because it depends on c-http and c-io, so they must follow
    println!("cargo:rustc-link-lib=static=aws-c-auth");
    println!("cargo:rustc-link-lib=static=aws-c-http");
    println!("cargo:rustc-link-lib=static=aws-c-io");

    // c-auth/c-http also use c-compression, c-sdkutils, c-cal, and c-common
    println!("cargo:rustc-link-lib=static=aws-c-compression");
    println!("cargo:rustc-link-lib=static=aws-c-sdkutils");
    println!("cargo:rustc-link-lib=static=aws-c-cal");
    println!("cargo:rustc-link-lib=static=aws-c-common");

    // s2n depends on libcrypto, so s2n should be linked before crypto
    println!("cargo:rustc-link-lib=static=s2n");
    println!("cargo:rustc-link-lib=static=crypto");

    // println!("cargo:rustc-link-lib=static=aws-nitro-enclaves-sdk-c");
    // println!("cargo:rustc-link-lib=static=aws-c-http");
    // println!("cargo:rustc-link-lib=static=aws-c-io");
    // println!("cargo:rustc-link-lib=static=aws-c-auth");
    // println!("cargo:rustc-link-lib=static=aws-c-common");
    // println!("cargo:rustc-link-lib=static=aws-c-cal");
    // println!("cargo:rustc-link-lib=static=s2n");
    // println!("cargo:rustc-link-lib=static=crypto");
    // println!("cargo:rustc-link-lib=static=aws-c-compression");
    // println!("cargo:rustc-link-lib=static=aws-c-sdkutils");
}