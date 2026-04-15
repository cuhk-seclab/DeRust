// Same TyKind: AddressOf creates RawPtr example
// https://doc.rust-lang.org/std/primitive.pointer.html

fn main() {
    // 1. &T, &mut T
    let value = 42;

    unsafe {
        let ptr = &value as *const f32; // Cannot
        println!("{}", *ptr);
    }

    // 2. Box<T> -> Box::into_raw

    // 3. std::ptr::addr_of!, std::ptr::addr_of_mut! -> no need to create Ref to RawPtr -> packed struct or uninitialized memory

    // 4. Slice: as_ptr, as_mut_ptr function
}
