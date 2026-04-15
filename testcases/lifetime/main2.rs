fn genvec() -> &'static [u8] {
    let mut s = String::from("A test string");
    let ptr = s.as_mut_ptr();
    unsafe {
        let slice = std::slice::from_raw_parts(ptr, s.len());
        println!("{:?}", slice);
        // std::mem::forget(s);
        slice
    }
}

fn main() {
    let v = genvec();
    println!("{:?}", v);

    unsafe {} // unsafe code block
}
