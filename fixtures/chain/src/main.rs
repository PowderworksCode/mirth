use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use base::{Label, Shape};

async fn call_once(f: impl AsyncFnOnce() -> usize) -> usize {
    f().await
}

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

fn main() {
    let squares = [mid::Square(3), mid::Square(5)];
    let largest = base::largest(&squares).expect("two squares");
    let rectangle = mid::Rectangle {
        width: 4,
        height: 6,
    };
    let clamped = base::old_clamp(250);
    let buffer: mid::Buffer = [0; _];
    let greeting = block_on(call_once(base::greeter("chain".to_string())));
    println!(
        "{} {} {} {} {} {} {} {}",
        largest.describe(),
        rectangle.area(),
        mid::total(&squares),
        clamped,
        buffer.len(),
        greeting,
        base::Square(2).label(),
        base::base_limit(),
    );
}
