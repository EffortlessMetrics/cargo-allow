pub fn access(ptr: *const u8) -> u8 {
    unsafe { core::ptr::read(ptr) }
}
