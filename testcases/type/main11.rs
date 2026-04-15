// Same TyKind, RawPtr(I::TypeAndMut (Ty<'tcx>, Mutability)), Ref(I::Region, I::Ty, Mutability) example
use std::mem;

fn main() {
    // RawPtr
    // 1. Big to small
    // u32 -> u8/i8: truncation
    // let src: *const u32 = &257;
    // let dst: *const u8 = src as *const u8;

    // unsafe {
    //     println!("src: {:?}", *src);
    //     println!("dst: {:?}", *dst);
    // }

    // let dst: *const i8 = src as *const i8;
    // let dst: *const u32 = src as *const i8 as *const u8 as *const u32;

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
    let src_val: u8 = 42;
    // let dst: &u32 = &src_val as &u32; // NO
    let dst_val: u32 = src_val as u32;

    // let src: *const u8 = &42;
    // let dst: *const u32 = src as *const u32;
    // let dst: *const i32 = src as *const i32;
    // let dst: *const u32 = unsafe { std::mem::transmute::<*const u8, *const u32>(src) };
    // let dst_val = unsafe { std::mem::transmute::<&u8, &u32>(&src_val) };
    unsafe {
        // println!("src: {:?}", *src);
        // println!("dst: {:?}", *dst);
        println!("src: {:?}", src_val);
        println!("dst: {:?}", dst_val);
    }

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

    // 3. Float, Int, Uint
    // let src: *const f32 = &42.5;
    // let dst: *const u8 = src as *const u8;
    // unsafe {
    //     println!("src: {:?}", *src);
    //     println!("dst: {:?}", *dst);
    // }

    // 4. Sequence types
    // #[derive(Debug, Clone)]
    // struct Person<T> {
    //     // name: String,
    //     age: u32,
    //     data: T,
    // }

    // let src: Person<i32> = Person {
    //     // name: "Barry".to_string(),
    //     age: 24,
    //     data: 100,
    // };

    // Transmute
    // println!("src: {:?}", src.clone());
    // let dst: &u64 = unsafe { std::mem::transmute(src) };
    // println!("dst: {}", dst);

    // As
    // println!("src: {:?}", src.clone());
    // let dst: *const Person<u64> = &src as *const Person<i32> as *const Person<u64>;
    // let dst: *const u32 = &src as *const Person<i32> as *const u32;
    // unsafe {
    //     println!("dst: {:?}", *dst);
    // }

    // As
    // println!("src: {:?}", src.clone());
    // let dst: *const u64 = &src as *const Person<i32> as *const u64;
    // unsafe {
    //     println!("dst: {:?}", *dst);
    // }

    // 5. Reference types
    // Wrong
    // let val: f64 = 42.5;
    // let src: *const u32 = &val as *const u32; // casting `&f64` as `*const u32` is invalid
    // let src: *const &f64 = &val as *const f64 as *const &f64;
    // let src: *const *const f64 = &val as *const f64 as *const *const f64;
    // unsafe {
    //     println!("src: {:?}", *src);
    // }

    // Ref
    // a) Ty conversion: through RawPtr, through transmute/transmute_copy
    // 1. Big to small
    // u32 -> u8/i8: truncation
    // let src: &u32 = &300;
    // let dst: &u8 = unsafe { &*(src as *const u32 as *const u8) };
    // // let dst: &u8 = src as &u8;  // NO. An `as` expression can only be used to convert between primitive types or to coerce to a specific trait object
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // let src: &u32 = &257;
    // let dst: &u8 = unsafe { std::mem::transmute::<&u32, &u8>(src) };
    // let dst: &i8 = unsafe { std::mem::transmute::<&u32, &i8>(src) };
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // i32 -> u8/i8: flip and truncation
    // let src: &i32 = &(-257);
    // let dst: &u8 = unsafe { std::mem::transmute::<&i32, &u8>(src) };
    // let dst: &i8 = unsafe { std::mem::transmute::<&i32, &i8>(src) };
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // 2. Small to big
    // u8 -> u32/i32: ub
    // let src: &u8 = &42;
    // let dst: &u32 = unsafe { std::mem::transmute::<&u8, &u32>(src) };
    // let dst: &i32 = unsafe { std::mem::transmute::<&u8, &i32>(src) };
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // i8 -> u32/i32: flip and truncation
    // let src: &i8 = &(-42);
    // let dst: &u32 = unsafe { std::mem::transmute::<&i8, &u32>(src) };
    // let dst: &i32 = unsafe { std::mem::transmute::<&i8, &i32>(src) };
    // println!("src: {:?}", src);
    // println!("dst: {:?}", dst);

    // 3. Sequence types
    // #[derive(Debug, Clone)]
    // struct Person<T> {
    //     age: u32,
    //     data: T,
    // }

    // let src: Person<i32> = Person { age: 24, data: 100 };

    // Transmute
    // println!("src: {:?}", src.clone());
    // let dst: &u64 = unsafe { std::mem::transmute(&src) };
    // println!("dst: {}", dst);

    // b) Lifetime/Region conversion

    // unsafe {}
}
