// Same TyKind: FloatToFloat, FloatToInt, IntToFloat CastKind. Float, Int and Uint example

// FLASH: deprecated
fn main() {
    // Truncate cast
    let tmp: f64 = -42.5;
    let tmp_int: u32 = tmp as u32;
    println!("tmp as f64: {}", tmp);
    println!("tmp_int as u32: {}", tmp_int);

    // Accuracy lost
    let src: f64 = 42.5;
    let dst: f32 = src as f32;
    println!("{}", src);
    println!("{}", dst);

    // More: ...

    unsafe {}
}
