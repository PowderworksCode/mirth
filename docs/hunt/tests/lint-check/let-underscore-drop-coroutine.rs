#![feature(coroutines, stmt_expr_attributes)]
#![warn(let_underscore_drop)]

fn main() {
    let _ = #[coroutine] || yield 42;
}
