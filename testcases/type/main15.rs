// Generic and trait bound case
// Trait bound scenarios:
//      impl Traits
//              Function args
//              Return types: May not have generic but only trait bound
//      Trait objects
//              Static-dispatched trait objects
//              dyn trait objects -> fat pointers/vtables know the trait bounds?
//      Special cases: From/Into traits

use std::mem;

// Trait object as input parameters -> The final trait object would be concretized during runtime
// 1 Static-dispatched trait objects
/*
trait Shape {
    fn area(&self) -> f64;
}

struct Rectangle {
    width: f64,
    height: f64,
}

impl Shape for Rectangle {
    fn area(&self) -> f64 {
        self.width * self.height
    }
}

// fn print_area(shape: &impl Shape) {
fn print_area(shape: impl Shape) {
    // let rawptr_shape = &shape as &dyn Shape as *const dyn Shape;
    // let rawptr_shape = &shape as &Shape as *const Shape;
    let rawptr_shape = &shape as *const Shape;
    let val = rawptr_shape as *const u32;

    // let val = unsafe { mem::transmute::<&Shape, &Shape>(&shape) };

    unsafe {
        println!("Area: {}", shape.area());
        println!("val: {}", *val);
    }
}

fn main() {
    let rectangle = Rectangle {
        width: 5.0,
        height: 3.0,
    };

    print_area(rectangle);
    // print_area(&rectangle);

    unsafe {}
}
*/

// 2 dyn trait objects
// /*
trait Shape {
    fn area(&self) -> f64;
}

struct Rectangle {
    width: f64,
    height: f64,
}

impl Shape for Rectangle {
    fn area(&self) -> f64 {
        self.width * self.height
    }
}

struct Circle {
    radius: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        3.14 * self.radius * self.radius
    }
}

fn print_area(shape: &dyn Shape) {
    // let rawptr_shape = &shape as &dyn Shape as *const dyn Shape;
    // let rawptr_shape = &shape as &Shape as *const Shape;
    let rawptr_shape = shape as *const Shape;
    let val = rawptr_shape as *const u32;

    // let val = unsafe { mem::transmute::<&Shape, &Shape>(&shape) };

    unsafe {
        println!("Area: {}", shape.area());
        println!("val: {}", *val);
    }
}

fn main() {
    let rectangle = Rectangle {
        width: 5.0,
        height: 3.0,
    };

    let circle = Circle { radius: 2.0 };

    print_area(&rectangle as &dyn Shape); // Cast it as a dynamic trait object
    print_area(&circle as &dyn Shape);
}
// */
