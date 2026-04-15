fn type_conversion(src_ty: u8) -> u32 {
    // Alternative 1: As
    let tmp_ty = &src_ty as *const u8 as *const u32;
    unsafe {
        println!("{}", *tmp_ty);
        *tmp_ty
    }

    // Alternative 2: Transmute
    // unsafe { std::mem::transmute::<u8, u32>(src_ty) }
}

fn main() {
    let src_ty: u8 = 6;
    let dest_ty = type_conversion(src_ty);
}
