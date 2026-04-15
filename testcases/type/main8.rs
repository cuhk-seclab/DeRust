// Same TyKind: IntToInt, PointerCoercion CastKind. Char, Str, String, Uint example

// FLASH: ToDO: support std types (**std::string::String to const char/str**), prefix bbs/paths recursively occur...
fn main() {
    // 1. Char -> Uint/Unicode
    // char -> u32
    // let tmp: char = 'A';
    // let tmp_int: u32 = tmp as u32;
    // println!("tmp as char: {}", tmp);
    // println!("tmp_int as u32: {}", tmp_int);

    // 2. String -> Char -> String
    // let tmp_string: String = String::from("Hello");
    // let tmp_char: Vec<char> = tmp_string.chars().collect();
    // let tmp_new_string: String = tmp_char.iter().collect();
    // println!("tmp_string: {}", tmp_string);
    // println!("tmp_char: {:?}", tmp_char);
    // println!("tmp_new_string: {}", tmp_new_string);

    // 3. String -> Vec<Uint/Unicode> -> String

    // 4. String to str: str = &string
    let tmp_string: String = String::from("Hello"); // MIR: _1 = <String as From<&str>>::from(const "Hello")
                                                    // let tmp_string = "Hello"; // MIR: const
    let tmp_str: &str = &tmp_string;
    // let tmp_str: &str = &tmp_string[..2]; // MIR: _3 = <str as Index<RangeTo<usize>>>::index(move _4, move _5)
    println!("tmp_string: {}", tmp_string);
    println!("tmp_str: {}", tmp_str);

    // str to String: str.to_string()
    // let tmp_str: &str = "World"; // MIR: const str
    // let tmp_string: String = tmp_str.to_string(); // MIR: std::string::String
    // println!("tmp_str: {}", tmp_str);
    // println!("tmp_string: {}", tmp_string);

    // 5.
    // String to Cow<str>: Cow<str> = string.into()
    // Cow<str> to String: Cow<str>.into_owned()

    unsafe {}
}
