use super::{AWSAllocator, aws_byte_buf_clean_up};

use std::alloc::Layout;

/// A buffer of bytes.
/// 
/// This type is a direct mapping of the `aws_byte_buf` type in the AWS C Common library.
/// 
/// <https://github.com/awslabs/aws-c-common/blob/9fd58f977d5779f8a695dd963e75cf3abee8231e/include/aws/common/byte_buf.h#L27>
#[repr(C)]
pub struct AWSByteBuf {
    len: usize,
    buffer: *mut u8,
    capacity: usize,
    allocator: *mut AWSAllocator,
}

impl From<Vec<u8>> for AWSByteBuf {
    fn from(value: Vec<u8>) -> Self {
        Self::from(value.as_slice())
    }
}

impl From<&[u8]> for AWSByteBuf {
    fn from(value: &[u8]) -> Self {
        debug_assert!(value.len() < isize::MAX as usize);

        // For simplicity, we only allocate exactly the amount of memory needed.
        let len = value.len();
        
        let buffer = unsafe {
            // SAFTEY: 
            // - Align comes from the `u8` type, and is therefore valid.
            // - Size is assumed to be valid as it comes from the slice.
            let layout = Layout::from_size_align_unchecked(len, std::mem::align_of::<u8>());

            // Ptr is assumed to be non-null.
            let buffer = std::alloc::alloc(layout);

            // SAFETY:
            // - `len` comes directly from the slice.
            // - `buffer` is allocated with the len from the slice.
            // - any alignment is valid for `u8`.
            // - The allocator is assumed to not give a overlapping region for `buffer`.
            std::ptr::copy_nonoverlapping(value.as_ptr(), buffer, len);

            buffer
        };

        AWSByteBuf {
            len,
            buffer,
            capacity: len,
            allocator: std::ptr::null_mut(),
        }
    }
}

impl Drop for AWSByteBuf {
    fn drop(&mut self) {
        if self.allocator.is_null() {
            // SAFETY: 
            // - By the invariants of this type, if the allocator is null, 
            //   the buffer was allocated by Rust code.
            // 
            // - The capacity is assumed to be correct
            // 
            // - The buffer is assumed to be properly aligned, since its created by Rust code.
            // 
            // - `u8` has noop destructor, no need to call drop.
            unsafe {
                let layout = Layout::from_size_align_unchecked(self.len, std::mem::align_of::<u8>());

                std::alloc::dealloc(self.buffer, layout);
            }
        } else {
            // SAFTEY: The buffer was allocated by the AWSAllocator as its nonnull.
            unsafe {
                aws_byte_buf_clean_up(self);
            }
        }
    }
}