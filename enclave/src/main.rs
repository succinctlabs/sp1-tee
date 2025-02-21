mod server;
mod executor;
mod ffi;

fn main() {
    unsafe { ffi::aws_nitro_enclaves_library_init(std::ptr::null_mut()); }

    println!("Hello, world!");

    loop {}
}