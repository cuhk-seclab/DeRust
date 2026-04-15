// Same TyKind, Array, Tuple example
// a) Whole Array, Tuple cases
// b) Array, Tuple field cases

// ToDO: Different TyKind with Array, Tuple
//       Whole Array, Tuple with primitive type cases
//       Array, Tuple field cases with primitive type cases

fn main() {
    // vec![] and Array are different
    // let mut vec: Vec<i32> = vec![1, 2, 3];
    // vec.push(4);
    // println!("{:?}", vec);

    // let mut vec: Vec<dyn std::any::Any> = vec![Box::new(42), Box::new(3.14), Box::new(true)];
    // println!("{:?}", vec);

    // let boxed_array: Box<[i32]> = Box::new([1, 2, 3]);

    // Array
    // let src: [i32; 4] = [1, 2, 3, 4];

    // println!("src: {:?}", src);
    // let dst: [i32; 4] = src as [i32; 4];
    // println!("dst: {:?}", dst);

    // println!("src: {:?}", src);
    // let dst = unsafe { std::mem::transmute::<[i32; 4], [u32; 4]>(src) };
    // println!("dst: {:?}", dst);

    // Tuple
    let src: (i32, f64) = (42, 3.14);

    // Tuple elements direct conversion
    let dst: (i64, f32);
    unsafe {
        dst = std::mem::transmute(src);
    }
    println!("src: {:?}", src);
    println!("dst: {:?}", dst);

    // Wrong: An `as` expression can only be used to convert between primitive types or to coerce to a specific trait object.
    // println!("src: {:?}", src);
    // let dst: (i32, f64) = src as (i32, f64);
    // println!("dst: {:?}", dst);

    unsafe {}
}
