use base::Shape;

fn main() {
    let squares = [mid::Square(3), mid::Square(5)];
    let largest = base::largest(&squares).expect("two squares");
    let rectangle = mid::Rectangle {
        width: 4,
        height: 6,
    };
    let clamped = base::old_clamp(250);
    let buffer: mid::Buffer = [0; _];
    println!(
        "{} {} {} {} {}",
        largest.describe(),
        rectangle.area(),
        mid::total(&squares),
        clamped,
        buffer.len(),
    );
}
