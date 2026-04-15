// More generic scenario examples
// Generic type conversion can only happen using RawPtr

trait Shape {
    fn area(&self) -> f64;
}

#[derive(Debug, Clone)]
struct Rectangle {
    width: f64,
    height: f64,
}

impl Shape for Rectangle {
    fn area(&self) -> f64 {
        self.width * self.height
    }
}

fn print_area<T: Shape, U: Shape>(shape: T) -> *const U {
    let rawptr_shape = &shape as *const T;
    let rawptr = rawptr_shape as *const U;

    unsafe {
        // println!("rawptr val: {:?}", *rawptr_shape);
        // println!("Area: {}", U.area());
        // println!("val: {}", val);
    }
    rawptr
}

// fn print_area_2<U: Shape>(shape: u32) -> *const U {
//     let rawptr_shape = &shape as *const u32;
//     let rawptr = rawptr_shape as *const U;

//     unsafe {
//         println!("rawptr val: {}", *rawptr_shape);
//         // println!("Area: {}", U.area());
//         // println!("val: {}", val);
//     }
//     rawptr
// }

// fn print_area_3<T: Shape + Clone>(shape: T) -> *const u32 {
//     let rawptr_shape = &shape as *const T;
//     let rawptr = rawptr_shape as *const u32;
//     // let val = rawptr_shape as *const Shape; // Same/Similar type conversion would be mir::PointerCoercion

//     unsafe {
//         println!("Area: {}", shape.area());
//         println!("rawptr val: {}", *rawptr);
//         // println!("val: {}", val);
//     }
//     rawptr
// }

fn main() {
    let rectangle = Rectangle {
        width: 5.0,
        height: 3.0,
    };
    let src: u32 = 6;

    let dst: *const Rectangle = print_area(rectangle);
    // let dst: *const Rectangle = print_area_2(src);
    // let dst: *const u32 = print_area_3(rectangle);

    unsafe {
        // println!("dst: {}", *dst);
        println!("dst: {:?}", *dst);
    }
}
