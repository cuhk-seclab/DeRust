// Inter-procedural case
// ToDO:
//      Currently only support two functions with unsafe code blocks
//      Construct a call sequence? -> Start from the main functions, etc. -> Recursion problems -> Then no need to analyze independent functions?
//      Analyze individual functions -> Which paths would be the function summary or taint analysis choose? -> Analyze all paths are too expensive!
//                                   -> Restrict the call sequence length?

// Pros:
//      generic intra- and inter- differences -> inter- calling sequences could analyze the potential issues that the generic would bring

// ToDO: test func1 -> func2 -> func3

use std::mem;

fn type_conversion(src: *const u8) -> *const u32 {
    unsafe {}

    src as *const u32
}

fn middle(src: *const u8) -> *const u32 {
    unsafe {}

    type_conversion(src)
}

fn main() {
    // RawPtr
    // 1. Big to small
    // u32 -> u8/i8: truncation
    let src: *const u8 = &6;
    // let dst: *const u8 = type_conversion(src);
    let dst: *const u32 = middle(src);
    unsafe {
        println!("src: {:?}", *src);
        println!("dst: {:?}", *dst);
    }

    unsafe {}
}

// Rest of the code...
/*
fn main() {
    // RawPtr
    // 1. Big to small
    // u32 -> u8/i8: truncation
    let src: *const u32 = &257;
    let dst: *const u8 = src as *const u8;
    // let dst: *const i8 = src as *const i8;
    unsafe {
        println!("src: {:?}", *src);
        println!("dst: {:?}", *dst);
    }

    // i32 -> u8/i8: flip and truncation
    // let src: *const i32 = &(-257);
    // let dst: *const u8 = src as *const u8;
    // let dst: *const i8 = src as *const i8;
    // unsafe {
    //     println!("src: {:?}", *src);
    //     println!("dst: {:?}", *dst);
    // }

    // 2. Small to big
    // u8 -> u32/i32: ub
    // let src: *const u8 = &42;
    // let dst: *const u32 = src as *const u32;
    // let dst: *const i32 = src as *const i32;
    // unsafe {
    //     println!("src: {:?}", *src);
    //     println!("dst: {:?}", *dst);
    // }

    // let src: *const u64 = &42;
    // let dst: *const u128 = src as *const u128;
    // unsafe {
    //     println!("src: {:?}", *src);
    //     println!("dst: {:?}", *dst);
    // }

    // i8 -> u32: flip and ub
    // let src: *const i8 = &(-42);
    // let dst: *const u32 = src as *const u32;
    // let dst: *const i32 = src as *const i32;
    // unsafe {
    //     println!("src: {:?}", *src);
    //     println!("dst: {:?}", *dst);
    // }
}
 */
