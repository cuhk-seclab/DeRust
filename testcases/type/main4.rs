// Different TyKind: Bool, Int, example

// Switches based on the computed value.
// First, evaluates the `discr` operand. The type of the operand must be a signed or unsigned integer, char, or bool, and must match the given type.

// ToDO: test control flow
fn main() {
    // // 6 7
    // let src_ty: u8 = 6;
    // Compile error: non-primitive cast
    // let dest_ty: u32 = &src_ty as u32;
    // let dest_ty: u32 = src_ty as u32;

    // Alternative 1: As
    // let tmp_ty = &src_ty as *const u8 as *const bool; // pointer: *const, *mut
    // let dest_ty = unsafe { &*tmp_ty };
    // let alias_ty = tmp_ty;

    // 6 7
    let src_ty: u8 = 6;
    // Alternative 2: Transmute
    // let dest_ty = unsafe { std::mem::transmute::<&u8, &bool>(&src_ty) }; // reference: &
    let dest_ty = &src_ty as *const u8 as *const bool;

    unsafe {
        // Undefined behavior
        println!("{:?}", *dest_ty);
        // println!("{:?}", *dest_ty == true);

        let condition = *dest_ty;
        // if (*dest_ty == true) {
        if (condition) {
            println!("If: {:?}", *dest_ty);
            // println!("src_ty is true");
            println!("Go into if branch.");
        } else {
            println!("Else: {:?}", *dest_ty);
            println!("Go into else branch.");
        }
    }

    unsafe {}
}
