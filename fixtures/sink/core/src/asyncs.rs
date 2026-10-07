//! async fn, async blocks, async closures, async fn in traits, a tiny executor.

use core::future::Future;
use core::pin::{Pin, pin};
use core::task::{Context, Poll, Waker};

pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// Yields once before finishing.
#[derive(Debug, Default)]
pub struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            Poll::Pending
        }
    }
}

pub async fn add_later(a: u32, b: u32) -> u32 {
    YieldOnce::default().await;
    a + b
}

#[allow(async_fn_in_trait)]
pub trait Fetch {
    async fn fetch(&self, key: &str) -> Option<String>;
}

#[derive(Debug)]
pub struct Static(pub &'static [(&'static str, &'static str)]);

impl Fetch for Static {
    async fn fetch(&self, key: &str) -> Option<String> {
        YieldOnce::default().await;
        self.0.iter().find(|(k, _)| *k == key).map(|(_, v)| v.to_string())
    }
}

/// An async closure that borrows what it captures.
pub fn greeter(name: String) -> impl AsyncFn(&str) -> String {
    async move |greeting: &str| format!("{greeting}, {name}")
}

pub async fn call_twice(f: impl AsyncFn(&str) -> String) -> String {
    let a = f("hello").await;
    let b = f("bye").await;
    format!("{a}; {b}")
}

pub fn boxed_future(n: u32) -> Pin<Box<dyn Future<Output = u32> + Send>> {
    Box::pin(async move {
        YieldOnce::default().await;
        n * 2
    })
}
