// More generic scenario examples
//      ToDO: try to support more cases

// 1 Adt types
//      General case
struct Pair<T> {
    first: usize,
    second: T,
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}

//      Impl case
use std::fmt::Debug;

struct Point<T> {
    x: T,
    y: T,
}

impl<T: Debug> Debug for Point<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Point {{ x: {:?}, y: {:?} }}", self.x, self.y)
    }
}

fn main() {
    let int_point = Point { x: 5, y: 10 };
    let float_point = Point { x: 1.5, y: 2.5 };

    println!("{:?}", int_point);
    println!("{:?}", float_point);
}

// 2 Trait with generic
trait Container<T> {
    fn contains(&self, item: T) -> bool;
}

struct List<T> {
    elements: Vec<T>,
}

impl<T: PartialEq> Container<T> for List<T> {
    fn contains(&self, item: T) -> bool {
        self.elements.contains(&item)
    }
}

// 3 Function case
//      General case
fn swap<T>(a: &mut T, b: &mut T) {
    let temp = *a;
    *a = *b;
    *b = temp;
}

//      Where predicate case
fn find_max<T>(list: &[T]) -> Option<T>
where
    T: PartialOrd + Copy,
{
    if list.is_empty() {
        None
    } else {
        let mut max = &list[0];
        for item in list.iter() {
            if item > max {
                max = item;
            }
        }
        Some(*max)
    }
}

fn main() {
    let numbers = vec![1, 5, 2, 10, 3];
    let max_number = find_max(&numbers);
    println!("Maximum number: {:?}", max_number);
}

// 4 std types
use std::fmt::Debug;

fn print_debug_elements<T: Debug>(vec: Vec<T>) {
    for element in vec {
        println!("{:?}", element);
    }
}

fn main() {
    let vec1 = vec![1, 2, 3, 4, 5];
    let vec2 = vec!["hello", "world"];

    print_debug_elements(vec1);
    print_debug_elements(vec2);
}

fn process_map<K, V>(map: HashMap<K, V>) {
    for (key, value) in map {
        println!("Key: {:?}, Value: {:?}", key, value);
    }
}

fn main() {
    let vec = vec![1, 2, 3, 4, 5];
    process_vec(vec);

    let mut map = HashMap::new();
    map.insert("one", 1);
    map.insert("two", 2);
    map.insert("three", 3);
    process_map(map);
}

// 5 Smart pointer types
fn process_box<T>(boxed_value: Box<T>) {
    println!("Boxed value: {:?}", *boxed_value);
}

fn process_rc<T>(rc_value: Rc<T>) {
    println!("Rc value: {:?}", *rc_value);
}

fn process_arc<T>(arc_value: Arc<T>) {
    println!("Arc value: {:?}", *arc_value);
}

fn main() {
    let boxed_value: Box<i32> = Box::new(42);
    process_box(boxed_value);

    let rc_value: Rc<i32> = Rc::new(42);
    process_rc(rc_value.clone()); // Cloning Rc to share ownership

    let arc_value: Arc<i32> = Arc::new(42);
    process_arc(arc_value.clone()); // Cloning Arc to share ownership
}

// 6 Other types
fn process_option<T>(option: Option<T>) {
    match option {
        Some(value) => println!("Option value: {:?}", value),
        None => println!("Option is empty"),
    }
}

fn process_result<T, E>(result: Result<T, E>) {
    match result {
        Ok(value) => println!("Result value: {:?}", value),
        Err(error) => println!("Error: {:?}", error),
    }
}

fn main() {
    let some_value: Option<i32> = Some(42);
    process_option(some_value);

    let none_value: Option<i32> = None;
    process_option(none_value);

    let ok_result: Result<i32, &str> = Ok(42);
    process_result(ok_result);

    let err_result: Result<i32, &str> = Err("Error occurred");
    process_result(err_result);
}
