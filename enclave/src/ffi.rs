mod bytebuf;
use bytebuf::AWSByteBuf;

mod string;

#[repr(C)]
pub struct AWSAllocator {
    #[doc(hidden)]
    /// The internals of this type are not important.
    ///
    /// The enclave merely needs to hold a pointer to this type.
    _unused: [u8; 0],
}

// Nitro Enclave SDK functions.
extern "C" {
    pub fn aws_nitro_enclaves_library_init(allocator: *mut AWSAllocator);

    pub fn aws_nitro_enclaves_get_allocator() -> *mut AWSAllocator;
}

// Common functions.
extern "C" {
    pub fn aws_byte_buf_clean_up(buf: *mut AWSByteBuf);
}
