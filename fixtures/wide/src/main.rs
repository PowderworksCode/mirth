use std::collections::BTreeMap;
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use core_lib::{Named, Store};
use user::{Drawing, Layer};

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
    let drawing = Drawing::sample();
    let mut points = Vec::new();
    user::each_point(&drawing, |p| points.push(p));
    let memory = core_lib::Memory(BTreeMap::from([("a".to_string(), 7)]));
    println!(
        "{:?} {} {} {} {} {} {:?} {}",
        drawing.describe(),
        drawing.pairs(),
        Drawing::NAME,
        core_lib::spread(points),
        user::Canvas::AREA,
        user::canvas().cells[0][0],
        user::layers().map(|(l, _)| l).max().unwrap_or(Layer::Back),
        block_on(memory.load("a")).unwrap_or(0),
    );
}
