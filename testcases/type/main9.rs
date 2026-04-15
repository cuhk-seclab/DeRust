// Same TyKind: Adt - Algebraic data type. Structure and Union only have one variant, Enumeration has multiple variants and fields
// a) Whole Adt cases:
//          1) Normal case, 2) Generic case,
//          3) repr case: repr(u*), repr(i*), repr(align(n)), repr(packed), repr(transparent); repr(C) -> Change the padding of the Adt, not Adt type layout
// b) Adt field cases: Similar to the simple case
// - Structures: e.g., List<i32> -> AdtDef: struct List<T>, args: [i32];
// - Enumerations
// - Unions

// ToDO: Different TyKind with Adt
//       Whole Adt cases with primitive types, etc.
//       Adt field cases with primitive types, etc.

// FLASH: ToDO: support Vec<T>
fn main() {
    // 1. Structures 1
    // #[derive(Debug, Clone)]
    // struct List<T> {
    //     elements: Vec<T>,
    // }

    // impl<T> List<T> {
    //     fn from_vec(elements: Vec<T>) -> List<T> {
    //         List { elements }
    //     }
    // }

    // let src: List<i32> = List {
    //     elements: vec![1, 2, 3], // allocation order: sizeof, alignof -> slice::into_vec
    // };

    // b) Field-sensitive process
    // let dst: List<i32> = List {
    //     elements: vec![4, 5, 6],
    // };

    // let dst_from_src: List<i32> = List {
    //     elements: src.elements.clone(),
    // };

    // c) From/To traits
    // let dst: List<i32> = List::from_vec(vec![4, 5, 6]);
    // println!("{:?}", src.clone());
    // println!("{:?}", dst);

    // d) Transmute
    // println!("{:?}", src.elements.clone());
    // let dst: List<u32> = unsafe { std::mem::transmute(src) }; // Just move between src and dst
    // let dst: i32 = unsafe { std::mem::transmute(src) }; // Cannot transmute should between types of different sizes, or dependently-sized types
    // println!("{:?}", dst.elements); // FLASH: ToDO: field-sensitive sink operations!
    // println!("{:?}", dst);

    // e) As cast is not allowed for non-primitive types
    // println!("{:?}", src.elements.clone());
    // let dst: List<i32> = src as List<i32>; // Just move between src and dst
    // println!("{:?}", dst.elements);

    // 1. Structures 2
    #[derive(Debug, Clone)]
    struct Person<T> {
        name: String,
        // age: u32,
        age: *const i32,
        data: T,
    }

    let mut src: Person<i32> = Person {
        name: "Barry".to_string(),
        // age: 24,
        age: &24 as *const i32, // Construction stmt not works
        data: 100,
    };

    unsafe {
        println!("{:?}", *src.age);
    }

    // src.age = src.age as *const u8 as *const i32;
    // src.age = &(-300) as *const i32 as *const u8 as *const i32;
    let dst: *const Person<u8> = &src as *const Person<i32> as *const Person<u8>;

    // Transmute
    println!("{:?}", src.clone());
    // let dst: Person<u32> = unsafe { std::mem::transmute::<Person<i32>, Person<u32>>(src) };
    // println!("{:?}", dst.data); // FLASH: ToDO: field-sensitive sink operations!
    // println!("{:?}", dst);
    unsafe {
        // println!("{:?}", *src.age);
        println!("{:?}", *dst);
    }

    // As
    // println!("{:?}", src.clone());
    // let dst: Person<i32> = src as Person<i32>;
    // println!("{:?}", dst);

    // 1. Structures 3
    // #[repr(i16)]
    // struct Temperature(i16);
    // let temp = Temperature(-10);

    // 1. Structures 4
    // #[repr(align(32))]
    // #[derive(Debug, Clone)]
    // struct AlignedStruct {
    //     field1: u8,
    //     field2: u16,
    //     field3: u32,
    // }

    // let aligned = AlignedStruct {
    //     field1: 1,
    //     field2: 2,
    //     field3: 3,
    // };

    // Transmute
    // println!("{:?}", aligned);
    // let dst: AlignedStruct =
    //     unsafe { std::mem::transmute::<AlignedStruct, AlignedStruct>(aligned) };
    // println!("{:?}", dst);

    // As
    // println!("{:?}", aligned);
    // let dst: AlignedStruct = aligned as AlignedStruct;
    // println!("{:?}", dst);

    // 1. Structures 5
    // #[repr(packed)]
    // struct PackedStruct {
    //     field1: u8,
    //     field2: u16,
    //     field3: u8,
    // }

    // let packed = PackedStruct {
    //     field1: 10,
    //     field2: 100,
    //     field3: 5,
    // };

    // 1. Structures 6
    // #[repr(transparent)]
    // struct TransparentStruct(u32);

    // let transparent = TransparentStruct(42);

    // As
    // println!("{:?}", src.clone());
    // let dst: Person<i32> = src as Person<i32>;
    // println!("{:?}", dst);

    // ======================================== //

    // 2. Enumerations 1
    // enum Option<T> {
    //     Some(T),
    //     None,
    // }

    // enum List<T> {
    //     Cons(T, Box<List<T>>),
    //     Nil,
    // }

    // #[derive(Debug)]
    // enum Shape {
    //     Circle { radius: f64 },
    //     Rectangle { width: f64, height: f64 },
    //     Triangle { base: f64, height: f64 },
    // }

    // a) As cast is not allowed for non-primitive types
    // b) Transmute
    // let src: Shape = Shape::Rectangle {
    //     width: 5.0,
    //     height: 6.0,
    // };
    // println!("{:?}", src);
    // let dst: Shape = unsafe { std::mem::transmute(src) };
    // println!("{:?}", dst);

    // 2. Enumerations 2
    // #[repr(u8)]
    // enum Color {
    //     Red = 1,
    //     Green = 2,
    //     Blue = 3,
    // }
    // let color = Color::Red;

    // 3. Unions
    // union Data {
    //     number: i32,
    //     boolean: bool,
    // }

    // union Color<T, U> {
    //     rgba: [T; 4],
    //     hex: U,
    // }

    unsafe {}
}
