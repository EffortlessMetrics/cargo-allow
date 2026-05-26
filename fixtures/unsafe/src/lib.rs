pub unsafe fn read_byte(ptr: *const u8) -> u8 {
    unsafe { core::ptr::read(ptr) }
}
