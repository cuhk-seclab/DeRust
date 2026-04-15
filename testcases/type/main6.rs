// Same TyKind: IntToInt CastKind. Int and Uint example

// Rustc compiler supports this type conversion by truncation or flipping the negative values.
// Developers should take responsibility of these behaviors. Depend on **intentional or unintentional** behaviors.
// -> Different from RawPtr conversion, since RawPtr would not handle the adjacent address.
fn main() {
    // 1. Big to small
    // Rust arithmetic wrapping/truncation: https://doc.rust-lang.org/std/num/struct.Wrapping.html
    // Rustc lint for type conversion debug: https://doc.rust-lang.org/stable/nightly-rustc/rustc_lint/builtin/static.TRIVIAL_NUMERIC_CASTS.html

    // u32 -> u8/i8. Truncate cast
    // let src: u32 = 257;
    // let dst: u8 = src as u8; // 1, src % 256
    // let dst: i8 = src as i8; // 1, src % 256
    // println!("{}", src);
    // println!("{}", dst);

    // i32 -> u8/i8. Flip and truncate
    // let src: i32 = -420;
    // let dst: u8 = src as u8; // 214
    // let dst: i8 = src as i8;
    // println!("{}", src);
    // println!("{}", dst);

    // 2. Small to big
    // u8 -> u32/i32: Correct
    // let src: u8 = 42;
    // let dst: u32 = src as u32; // 42
    // let dst: i32 = src as i32; // 42
    // println!("{}", src);
    // println!("{}", dst);

    // i8 -> u32/i32: Wrong: Flip
    // let src: i8 = -42;
    // let dst: u32 = src as u32; // 4294967254
    // let dst: i32 = src as i32; // -42
    // println!("{}", src);
    // println!("{}", dst);

    unsafe {}
}
