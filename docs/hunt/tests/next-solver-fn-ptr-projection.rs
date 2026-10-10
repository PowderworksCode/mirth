pub trait AddLifetime: 'static { type Of<'a>; }
pub trait FTarget1<A, R, T> { fn run(a: A, t: T) -> R; }
#[derive(Clone, Copy)] pub struct V;
impl V {
    pub unsafe fn run_function_with_1<F, T0, R>(self, x0: T0::Of<'_>) -> R
    where T0: AddLifetime, F: for<'a> FTarget1<Self, R, T0::Of<'a>> { F::run(self, x0) }

    pub fn dispatch1<F, R, T0>(self) -> unsafe fn(Self, T0::Of<'_>) -> R
    where T0: AddLifetime, F: for<'a> FTarget1<Self, R, T0::Of<'a>> {
        let f: unsafe fn(Self, T0::Of<'_>) -> R = Self::run_function_with_1::<F, _, _>;
        f
    }
}
fn main() {}
