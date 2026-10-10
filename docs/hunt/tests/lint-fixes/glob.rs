mod m { pub struct S {} }
mod one_private {
    use crate::m::*;
    pub use crate::m::*;
}
use crate::one_private::S;
fn main() { let _ = S {}; }
