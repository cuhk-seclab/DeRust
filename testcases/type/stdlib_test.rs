#![feature(layout_for_ptr)]
use std::mem;

fn main() {
    let src_ty: u8 = 6;

    // Alternative 1: As
    let tmp_ty = &src_ty as *const u8 as *const u32; // pointer
    let dest_ty = unsafe { &*tmp_ty }; // reference

    // Alternative 2: Transmute
    // let dest_ty = unsafe { std::mem::transmute::<&u8, &u32>(&src_ty) }; // reference

    println!("{}", dest_ty);
    println!("{}", *dest_ty);
    unsafe {
        println!("{}", *tmp_ty);
    }

    // struct MyStruct {
    //     field1: u8,
    //     field2: u16,
    // }

    println!("size_of::<u8>: {}", mem::size_of::<u8>());
    println!("size_of::<u16>: {}", mem::size_of::<u16>());
    println!("size_of::<u32>: {}", mem::size_of::<u32>());
    println!("size_of::<u64>: {}", mem::size_of::<u64>());
    println!("size_of::<u128>: {}", mem::size_of::<u128>());
    println!("======");
    println!("align_of::<u8>: {}", mem::align_of::<u8>());
    println!("align_of::<u16>: {}", mem::align_of::<u16>());
    println!("align_of::<u32>: {}", mem::align_of::<u32>());
    println!("align_of::<u64>: {}", mem::align_of::<u64>());
    println!("align_of::<u128>: {}", mem::align_of::<u128>());
    println!("======");
    println!("size_of::<i8>: {}", mem::size_of::<i8>());
    println!("size_of::<i16>: {}", mem::size_of::<i16>());
    println!("size_of::<i32>: {}", mem::size_of::<i32>());
    println!("size_of::<i64>: {}", mem::size_of::<i64>());
    println!("size_of::<i128>: {}", mem::size_of::<i128>());
    println!("======");
    println!("align_of::<i8>: {}", mem::align_of::<i8>());
    println!("align_of::<i16>: {}", mem::align_of::<i16>());
    println!("align_of::<i32>: {}", mem::align_of::<i32>());
    println!("align_of::<i64>: {}", mem::align_of::<i64>());
    println!("align_of::<i128>: {}", mem::align_of::<i128>());
    println!("======");
    println!("size_of::<f32>: {}", mem::size_of::<f32>());
    println!("size_of::<f64>: {}", mem::size_of::<f64>());
    println!("======");
    println!("align_of::<f32>: {}", mem::align_of::<f32>());
    println!("align_of::<f64>: {}", mem::align_of::<f64>());
    println!("======");
    println!("size_of::<bool>: {}", mem::size_of::<bool>());
    println!("align_of::<bool>: {}", mem::align_of::<bool>());
    println!("======");
    println!("size_of::<char>: {}", mem::size_of::<char>());
    println!("align_of::<char>: {}", mem::align_of::<char>());
    // println!("======");
    // println!("size_of::<str>: {}", mem::size_of::<str>());
    // println!("align_of::<str>: {}", mem::align_of::<str>());
    println!("======");
    println!("size_of::<usize>: {}", mem::size_of::<usize>());
    println!("align_of::<usize>: {}", mem::align_of::<usize>());

    println!("======");
    println!("======");
    println!("======");
    println!("======");

    // References of the primitive types are all 8.
    println!("&u8 size_of {}", mem::size_of::<&u8>());
    println!("&u16 size_of {}", mem::size_of::<&u16>());
    println!("&u32 size_of {}", mem::size_of::<&u32>());
    println!("&u64 size_of {}", mem::size_of::<&u64>());
    println!("&u128 size_of {}", mem::size_of::<&u128>());
    println!("======");
    println!("&u8 align_of {}", mem::align_of::<&u8>());
    println!("&u16 align_of {}", mem::align_of::<&u16>());
    println!("&u32 align_of {}", mem::align_of::<&u32>());
    println!("&u64 align_of {}", mem::align_of::<&u64>());
    println!("&u128 align_of {}", mem::align_of::<&u128>());
    println!("======");
    println!("&i8 size_of {}", mem::size_of::<&i8>());
    println!("&i16 size_of {}", mem::size_of::<&i16>());
    println!("&i32 size_of {}", mem::size_of::<&i32>());
    println!("&i64 size_of {}", mem::size_of::<&i64>());
    println!("&i128 size_of {}", mem::size_of::<&i128>());
    println!("======");
    println!("&i8 align_of {}", mem::align_of::<&i8>());
    println!("&i16 align_of {}", mem::align_of::<&i16>());
    println!("&i32 align_of {}", mem::align_of::<&i32>());
    println!("&i64 align_of {}", mem::align_of::<&i64>());
    println!("&i128 align_of {}", mem::align_of::<&i128>());

    println!("======");
    println!("======");
    println!("======");
    println!("======");

    println!("*mut u8 size_of {}", mem::size_of::<*mut u8>());
    println!("*mut u16 size_of {}", mem::size_of::<*mut u16>());
    println!("*mut u32 size_of {}", mem::size_of::<*mut u32>());
    println!("*mut u64 size_of {}", mem::size_of::<*mut u64>());
    println!("*mut u128 size_of {}", mem::size_of::<*mut u128>());
    println!("======");
    println!("*mut u8 align_of {}", mem::align_of::<*mut u8>());
    println!("*mut u16 align_of {}", mem::align_of::<*mut u16>());
    println!("*mut u32 align_of {}", mem::align_of::<*mut u32>());
    println!("*mut u64 align_of {}", mem::align_of::<*mut u64>());
    println!("*mut u128 align_of {}", mem::align_of::<*mut u128>());
    println!("======");
    println!("*mut i8 size_of {}", mem::size_of::<*mut i8>());
    println!("*mut i16 size_of {}", mem::size_of::<*mut i16>());
    println!("*mut i32 size_of {}", mem::size_of::<*mut i32>());
    println!("*mut i64 size_of {}", mem::size_of::<*mut i64>());
    println!("*mut i128 size_of {}", mem::size_of::<*mut i128>());
    println!("======");
    println!("*mut i8 align_of {}", mem::align_of::<*mut i8>());
    println!("*mut i16 align_of {}", mem::align_of::<*mut i16>());
    println!("*mut i32 align_of {}", mem::align_of::<*mut i32>());
    println!("*mut i64 align_of {}", mem::align_of::<*mut i64>());
    println!("*mut i128 align_of {}", mem::align_of::<*mut i128>());

    println!("======");
    println!("======");
    println!("======");
    println!("======");

    let u8_ty: u8 = 6;
    let u16_ty: u16 = 6;
    let u32_ty: u32 = 6;
    let u64_ty: u64 = 6;
    let u128_ty: u128 = 6;
    println!("{}", mem::size_of_val(&u8_ty));
    println!("{}", mem::size_of_val(&u16_ty));
    println!("{}", mem::size_of_val(&u32_ty));
    println!("{}", mem::size_of_val(&u64_ty));
    println!("{}", mem::size_of_val(&u128_ty));
    println!("======");
    println!("{}", mem::align_of_val(&u8_ty));
    println!("{}", mem::align_of_val(&u16_ty));
    println!("{}", mem::align_of_val(&u32_ty));
    println!("{}", mem::align_of_val(&u64_ty));
    println!("{}", mem::align_of_val(&u128_ty));
    println!("======");
    println!("{}", mem::size_of_val(&src_ty)); // &u8
    println!("{}", mem::size_of_val(&tmp_ty)); // &*u32
    println!("{}", mem::size_of_val(dest_ty)); // &*u32
    println!("======");
    println!("{}", mem::align_of_val(&src_ty));
    println!("{}", mem::align_of_val(&tmp_ty));
    println!("{}", mem::align_of_val(dest_ty));

    // Variables (state-machine), Initial state, Type conversion state, Final used state (type inconsistency)
}
