fn genvec() -> Vec<u8> {
    let mut s = String::from("A test string");
    // let mut s = ManuallyDrop::new(String::from("A test string"));
    let ptr = s.as_mut_ptr();
    unsafe {
        let v = Vec::from_raw_parts(ptr, s.len(), s.len());
        println!("{:?}", v);
        // std::mem::forget(s);
        v
    }
}

fn main() {
    let v = genvec();
    println!("{:?}", v);
    // assert_eq!('a' as u8, v[0]);

    unsafe {}
}
