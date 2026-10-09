//! Edition 2015: syntax later editions removed. Anonymous trait parameters, bare trait
//! objects, `async`, `await`, `dyn` and `try` as identifiers.
#![allow(anonymous_parameters, bare_trait_objects, keyword_idents)]

pub trait Scale {
    fn scale(&self, u32) -> u32;
}

pub struct Twice;

impl Scale for Twice {
    fn scale(&self, x: u32) -> u32 {
        x * 2
    }
}

pub fn apply(s: &Scale, x: u32) -> u32 {
    s.scale(x)
}

pub fn identifiers() -> u32 {
    let async = 1;
    let await = 2;
    let dyn = 3;
    let try = 4;
    async + await + dyn + try
}
