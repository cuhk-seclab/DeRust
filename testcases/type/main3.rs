// Same TyKind: Big to small, RawPtr, Ref example
// Depend on the user cases, indexed out of bound
// Tracking: Creation based on size; Indexing

use std::convert::TryInto;

fn main() {
    // 1) Vec indexing
    // let n = 300; // 400
    //              // let p = &n as *const i32 as *const u8; // to unsigned type
    // let p = &n as *const i32 as *const i8; // to signed type
    // unsafe {
    //     // println!("p: {:?}", *p);

    //     let v = vec![1; *p as usize]; // vec![1; 300] -> _19 = std::vec::from_elem::<i32>(const 1_i32, move _20)

    //     // let p_ref = &*p;
    //     // let v = vec![1; p_ref.try_into().unwrap()]; // vec![1; 300]

    //     // let p_ref = &*p;
    //     // let v = vec![1; *p_ref as usize]; // vec![1; 300]

    //     println!("{}", v[*p as usize]); // 300
    // }

    // 2) Array indexing
    // let a = [1; 300]; // [1; 300]
    // println!("{}", a[299]); // 300

    // 3) Slice indexing
    // let slice = &[1, 2, 3, 4, 5];
    // let index = 3;
    // let value = slice[index];
    // println!("The value at index {} is: {}", index, value);

    // 4) Tuple indexing
    // let tuple = (10, 20, 30, 40);
    // let value = tuple.2;
    // println!("The value at index {} is: {}", index, value);

    unsafe {}
}
