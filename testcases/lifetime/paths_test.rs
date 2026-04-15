// Lifetime expand functions
// ToDO: Search based on rustc doc
// 1. https://doc.rust-lang.org/std/index.html?search=from_raw_parts
// 2. https://doc.rust-lang.org/std/index.html?search=as_ptr

use std::alloc::{alloc, Layout};
use std::ptr::NonNull;

fn main() {
    unsafe {
        // // 1) Strong bypasses
        // std::ptr::read(12 as *const i32);
        // (12 as *const i32).read();

        // Require same type
        // std::intrinsics::copy(12 as *const i32, 34 as *mut i32, 56);
        // std::intrinsics::copy_nonoverlapping(12 as *const i32, 34 as *mut i32, 56);
        // std::ptr::copy(12 as *const i32, 34 as *mut i32, 56);
        // std::ptr::copy_nonoverlapping(12 as *const i32, 34 as *mut i32, 56);

        // let src: *const i8 = &(-6) as *const i8;
        // let dst: *mut u8;
        // std::ptr::copy(src, dst, 1);
        // std::intrinsics::copy(src, dst, 1);
        // println!("{:?}", *src);
        // println!("{:?}", *dst);

        // vec![12, 34].set_len(5678);
        // std::vec::Vec::from_raw_parts(12 as *mut i32, 34, 56);

        // // 2) Weak bypasses
        // std::mem::transmute::<_, *mut i32>(12 as *const i32);

        // (12 as *mut i32).write(34);
        // std::ptr::write(12 as *mut i32, 34);

        // (12 as *const i32).as_ref(); // &*
        // (12 as *mut i32).as_mut();

        // let mut ptr = NonNull::new(1234 as *mut i32).unwrap();
        // ptr.as_ref();
        // ptr.as_mut();

        // [12, 34].get_unchecked(0);
        // [12, 34].get_unchecked_mut(0);

        // std::ptr::slice_from_raw_parts(ptr: *const T, len: usize) -> len: the number of elements, not the number of bytes
        // std::ptr::slice_from_raw_parts_mut(ptr: *mut T, len: usize)

        // std::ptr::slice_from_raw_parts(12 as *const i32, 34);
        // std::ptr::slice_from_raw_parts_mut(12 as *mut i32, 34);
        // std::slice::from_raw_parts(12 as *const i32, 34);
        // std::slice::from_raw_parts_mut(12 as *mut i32, 34);

        // // 3) Generic function call
        // std::intrinsics::drop_in_place(12 as *mut i32);
        // std::ptr::drop_in_place(12 as *mut i32);
        // (12 as *mut i32).drop_in_place();

        // 4) More sources and sinks
        // std::String, std::Vec, std::Box

        // a) std::String
        // let mut s = String::from("hello");
        // let s = String::from_raw_parts(s.as_mut_ptr(), s.len(), s.capacity());

        // let v = "🗻∈🌏".get_unchecked(0..4);
        // let mut v = String::from("🗻∈🌏");
        // let mut_v = v.get_unchecked_mut(0..4);

        // b) std::Vec
        // let x = vec![1, 2, 4].get_unchecked(1);
        // let mut x = vec![1, 2, 4];
        // let mut_x = x.get_unchecked_mut(1);

        // c) std::Box
        // let x = Box::new(5);
        // let ptr = Box::into_raw(x);
        // let x2 = Box::from_raw(ptr);

        // let ptr = alloc(Layout::new::<i32>()) as *mut i32;
        // In general .write is required to avoid attempting to destruct
        // the (uninitialized) previous contents of `ptr`, though for this
        // simple example `*ptr = 5` would have worked as well.
        // ptr.write(5);
        // let x = Box::from_raw(ptr);
    }
}
