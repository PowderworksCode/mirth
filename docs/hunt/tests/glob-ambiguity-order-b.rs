pub mod prelude {
    mod t { pub fn date_range() { println!("prelude::t") } }
    pub use self::t::*;
    pub use super::dsl::*;
}
pub mod dsl {
    mod range { pub fn date_range() { println!("dsl::range") } }
    pub use self::range::*;
    use super::prelude::*;
}
use dsl::*;
use prelude::*;
fn main() { date_range(); }
