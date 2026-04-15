// Different TyKind: Generic, Slice, RawPtr example
use std::slice;

// Array -> Slice -> RawPtr. Generic could be undefined
fn foo<T: std::fmt::Debug>(a: &mut [T]) -> &u32 {
    // Require 4-byte alignment.
    let p = a.as_mut_ptr() as *mut u32; // as_mut_ptr()
    unsafe {
        // println!("{:?}", *p);
        let s = slice::from_raw_parts_mut(p, 1);
        let x = s[0];
        println!("{}", x);
        &*p
    }
}

fn main() {
    let mut x = [1u8; 10];
    let v = foo(&mut x[1..9]);
    println!("{:?}", v);
}
