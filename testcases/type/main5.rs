// Different TyKind: Adt example

#[repr(align(2))]
#[derive(Copy, Clone, Debug)]
struct Padding {
    a: u8,
    b: u16,
    c: u8,
}
fn main() {
    let la = Padding {
        a: 10,
        b: 11,
        c: 12,
    };
    let mdbval = MdbValue::new_from_sized(&la);
    let res = i32::from_mdb_value(&mdbval);
    println!("{:?}", res);
}

// [repr(rust)], [repr(C)] differences
