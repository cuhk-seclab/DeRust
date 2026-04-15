// Test for for loops
use std::mem;

fn main() {
    // RawPtr
    // 1. Big to small
    // u32 -> u8/i8: truncation

    for i in 0..2 {
        let src: *const u32 = &257;
        let dst: *const u8 = src as *const u8;

        unsafe {
            println!("src: {:?}", *src);
            println!("dst: {:?}", *dst);
        }
    }

    unsafe {}
}
