// Four unsafe occurances: unsafe fn, unsafe blocks {}, unsafe trait, unsafe impl

// 1)
unsafe fn hello() {
    println!("This is an unsafe hello function.");
}

fn main() {
    unsafe {
        hello();
    }
}

// 2) unsafe trait and unsafe impl
// unsafe trait UnsafeTrait {
//     fn unsafe_method(&self);
// }

// struct MyType;

// unsafe impl UnsafeTrait for MyType {
//     fn unsafe_method(&self) {
//         // Unsafe code block
//         println!("This is an unsafe method implementation.");
//     }
// }

// fn main() {
//     let my_instance = MyType;
//     unsafe {
//         my_instance.unsafe_method();
//     }
// }

// 3) unsafe impl
// struct MyStruct;

// impl MyStruct {
//     unsafe fn unsafe_method(&self) {
//         // Unsafe code implementation
//     }
// }

// fn main() {
//     let my_struct = MyStruct;
//     unsafe {
//         my_struct.unsafe_method();
//     }
// }
