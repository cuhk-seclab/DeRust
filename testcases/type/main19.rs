// Continuous type conversion
// - Current implementation:
//      The continuous type conversion happens in aliases. -> The continuous behaviors would be captured.
//      E.g., SmallToBigUintToUint, BigToSmallUintToUint -> Can track correct continuous conversion.

// - Improve implementation
//      u8 -> u32, Need to record u8 type

use std::mem;

fn main() {
    // RawPtr
    // 1. Big to small
    // u32 -> u8/i8: truncation

    let src: *const u8 = &6;
    let dst: *const u8 = src as *const u32 as *const u8;

    unsafe {
        println!("src: {:?}", *src);
        println!("dst: {:?}", *dst);
    }

    unsafe {}
}
