// Same TyKind: Small to big, RawPtr, Ref example

fn main() {
    let src_ty: u8 = 6;
    // Compile error: non-primitive cast
    // let dest_ty: u32 = &src_ty as u32;  // Wrong
    // let dest_ty: u32 = src_ty as u32;

    // Alternative 1: As, RawPtr
    let tmp_ty = &src_ty as *const u8 as *const u32;
    let dest_ty = unsafe { &*tmp_ty };
    // let alias_ty = tmp_ty;

    // Alternative 2: Transmute
    // let dest_ty = unsafe { std::mem::transmute::<&u8, &u32>(&src_ty) };

    println!("{}", *dest_ty);

    unsafe {
        println!("{}", *tmp_ty);
        // println!("{}", *dest_ty);
        // println!("{}", *alias_ty);
    }

    // Variables (state-machine, lattice): Initial state, Type conversion state, Final used state
    // Type inconsistency: u8 -> u32 -> u8
}
