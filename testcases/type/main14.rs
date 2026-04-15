// Generic and trait bound case
// https://doc.rust-lang.org/reference/items/generics.html
// Generic scenarios:
//   a) Function args, return, where clauses
//
//   b)
//      Adt types
//      std types (Vec<T>, HashMap<K, V>)
//      Smart pointer types (Box<T>, Rc<T>, Arc<T>...)
//      Others: Option<T>, Result<T, E>
// Replace generic with all possible types within its trait bounds
//      -> Trait bound: -> collect trait bound from HIR into a set
//          -> **Function** args, return, where
//          -> **Impl** trait objects, adt type, std, smart pointer type, etc.
//              -> 1) Standard trait bounds (Copy, Clone, Debug, etc.) -> Replace primitive types -> Deprecated
//              -> 2) Find all types (including Adt, etc.) that impl the **trait object** -> Replace those implemented types
//      Corner case: types from external packages, using name string matching

// BugCheckers
//      -> Only the generic that correctly implements the trait, and performs correct/same type conversion, would not cause bugs
//              -> type generics implement trait objects
//              -> const generics, e.g., const N: i32 -> Deprecated
//      -> Others would cause bugs such as replacing using primitive, sequence, pointer, etc. types

use std::mem;

// 2. Function where clause
// T, U should implement From/Into traits -> Special functions
// fn convert_generic_trait<T, U>(value: T) -> U
// where
//     T: Into<U>,
// {
//     unsafe {}
//     value.into() // ToDO!! Special functions
//                  // Inter-procedural skips analysis: DefId::expect_local: `DefId(2:2555 ~ core[7a24]::convert::Into::into)` isn't local
// }

// 1. Function args, return
fn convert_generic_rawptr<T: std::fmt::Display, U: std::fmt::Display>(value: T) -> *const U {
    let ptr: *const T = &value as *const T as *const T;
    let ptr2: *const &T = &value as *const T as *const &T; // rawptr,ref case deprecated
    let ptr3: *const *const T = &value as *const T as *const *const T; //rawptr,rawptr case deprecated
    let raw_ptr: *const U = &value as *const T as *const U;
    unsafe {
        println!("src: {}", value);
        println!("ptr: {}", *ptr);
        println!("ptr2: {}", *ptr2);
        println!("ptr3: {}", **ptr3);
        println!("dst: {}", *raw_ptr);
    }
    raw_ptr
}

// fn convert_generic_rawptr_2<U: std::fmt::Display>(value: u8) -> *const U {
//     let raw_ptr: *const U = &value as *const u8 as *const U;
//     unsafe {
//         println!("src: {}", value);
//         println!("dst: {}", *raw_ptr);
//     }
//     raw_ptr
// }

// fn convert_generic_rawptr_3<T: std::fmt::Display + Clone>(value: T) -> *const u32 {
//     let raw_ptr: *const u32 = &value as *const T as *const u32;
//     unsafe {
//         println!("src: {}", value);
//         println!("dst: {}", *raw_ptr);
//     }
//     raw_ptr
// }

// fn convert_generic<T: std::fmt::Display>(value: &mut [T]) -> *const u32 {
//     let p = value.as_mut_ptr();
//     let raw_ptr: *const u32 = p as *const u32;
//     unsafe {
//         println!("p: {}", *p);
//         println!("raw_ptr: {}", *raw_ptr);
//     }
//     raw_ptr
// }

// 3. Function generic impl trait objects -> main15.rs

// Cannot transmute between types of different sizes, or dependently-sized types -> ❌
// fn convert_generic_transmute<T, U>(value: T) -> U {
//     unsafe { mem::transmute(value) }
// }

fn main() {
    let src: u8 = 42;

    // let dst: u32 = convert_generic_trait(src);
    let dst: *const u32 = convert_generic_rawptr(src);
    // let dst: *const u32 = convert_generic_rawptr_2(src);
    // let dst: *const u32 = convert_generic_rawptr_3(src);
    // let dst: *const u32 = convert_generic(&mut [src]);

    unsafe {
        println!("src: {}", src);
        println!("dst: {}", *dst);
        // println!("dst: {}", dst);
    }
}
