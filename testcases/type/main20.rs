// From, TryFrom, Into, TryInto

struct Rectangle {
    width: u32,
    height: u32,
}

impl Into<(u32, u32)> for Rectangle {
    fn into(self) -> (u32, u32) {
        (self.width, self.height)
    }
}

fn main() {
    let rectangle = Rectangle {
        width: 10,
        height: 20,
    };
    let dimensions: (u32, u32) = rectangle.into();
    println!("Width: {}, Height: {}", dimensions.0, dimensions.1);
}
