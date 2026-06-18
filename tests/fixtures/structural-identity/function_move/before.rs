fn read_left(ptr: *const u8) -> u8 {
    unsafe { core::ptr::read(ptr) }
}

fn read_right(ptr: *const u8) -> u8 {
    unsafe { core::ptr::read(ptr) }
}
