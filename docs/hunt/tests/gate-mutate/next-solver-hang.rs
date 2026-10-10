// A hang (no result in minutes) under the new trait solver; the old solver reports errors.
#![feature(type_alias_impl_trait)]
#![feature(closure_lifetime_binder)]
use std::future::Future;
trait AsyncFn<I, R>: FnMut(I) -> Self::Fut {
    type Fut: Future<Output = R>;
}
impl<F, I, R, Fut> AsyncFn<I, R> for F
where
    F: FnMut(I) -> Fut,
{
}
async fn call<C, R, F>(mut ctx: C, mut f: F) -> Result<R, ()>
where
    F: for<'a> AsyncFn<&'a mut C, Result<R, ()>>,
{
}
trait Cap<'a> {}
fn doesnt_work_but_should(ctx: &mut usize) {
    type Ret<'a, 'b: 'a> = impl Future<Output = Result<usize, ()>> + 'a + Cap<'b>;
    call(
        ctx,
        for<'a, 'b> |c: &'a mut &'b mut usize| -> Ret<'a, 'b> {
        },
    );
}
