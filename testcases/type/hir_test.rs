// Test HIR -> ItemKind

// 1. ItemKind::Impl
// struct Rectangle {
//     width: u32,
//     height: u32,
// }

// impl Rectangle {
//     fn area(&self) -> u32 {
//         self.width * self.height
//     }
// }

// fn main() {
//     let rectangle = Rectangle {
//         width: 10,
//         height: 5,
//     };
//     println!("The area of the rectangle is {}", rectangle.area());
// }

// 2. ItemKind::Trait
// trait Animal {
//     fn make_sound(&self);
// }

// struct Dog;
// struct Cat;

// impl Animal for Dog {
//     fn make_sound(&self) {
//         println!("Woof!");
//     }
// }

// impl Animal for Cat {
//     fn make_sound(&self) {
//         println!("Meow!");
//     }
// }

// fn main() {
//     let dog = Dog;
//     let cat = Cat;

//     dog.make_sound();
//     cat.make_sound();
// }

use std::fmt::Debug;

struct Point<T> {
    x: T,
    y: T,
}

impl<T: Debug> Debug for Point<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Point {{ x: {:?}, y: {:?} }}", self.x, self.y)
    }
}

fn main() {
    let int_point = Point { x: 5, y: 10 };
    let float_point = Point { x: 1.5, y: 2.5 };

    println!("{:?}", int_point);
    println!("{:?}", float_point);
}

// 3. ItemKind::Fn
// fn add(a: i32, b: i32) -> i32 {
//     a + b
// }

// fn main() {
//     let result = add(5, 3);
//     println!("The sum is {}", result);
// }
