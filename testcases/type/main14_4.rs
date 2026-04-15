// Test generic TyKind

#[derive(Debug, Clone, Copy)]
struct Pair<T> {
    first: T,
    second: T,
}

fn convert_generic_test<T: std::fmt::Debug + Copy>(a: T, b: T, c: T) {
    // let generic_arrs: [T; 3] = [a, b, c];
    // let generic_tuples: (T, T) = (a, b);
    // let generic_slice: &[T] = &generic_arrs[0..2];
    let generic_pair: Pair<T> = Pair {
        first: a,
        second: b,
    };

    // println!("{:?}", generic_arrs);
    // println!("{:?}", generic_tuples);
    // println!("{:?}", generic_slice);
    println!("{:?}", generic_pair);
}

fn main() {
    convert_generic_test::<u32>(1, 2, 3);
}
