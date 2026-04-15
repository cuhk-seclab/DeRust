// Same TyKind, Slice((I::Ty)), &mut [T] &[T] example
// ToDO: slice::from_raw_parts_mut and slice::from_raw_parts functions
use std;

fn main() {
    // 1. As: Wrong
    // let src: &[u8] = &[1, 2, 3, 4, 5];
    // let dst: &[u32] = src as &[u32]; // an `as` expression can only be used to convert between primitive types or to coerce to a specific trait object
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // 2. Transmute: the operation to address is undefined
    // let src: &[u8] = &[1, 2, 3, 4, 5];
    // let dst: &[u32] = unsafe { std::mem::transmute::<&[u8], &[u32]>(src) };
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // let src: &mut [u32] = &mut [257, 257, 257, 257, 257];
    // let dst: &mut [u8] = unsafe { std::mem::transmute::<&mut [u32], &mut [u8]>(src) };
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // 3. slice::from_raw_parts etc. functions: RawPtr -> Slice
    // let src: &[u8] = &[1, 2, 3, 4, 5];
    // let dst: &[i32] = unsafe {
    //     std::slice::from_raw_parts(
    //         src.as_ptr() as *const i32,
    //         src.len() / std::mem::size_of::<i32>(),
    //     )
    // };
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // let src: [u32; 5] = [1, 2, 3, 4, 5];
    // let ptr: *mut u8 = src.as_ptr() as *mut u8;
    // let len: usize = src.len();
    // let dst: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(ptr, len) };
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    unsafe {}
}
