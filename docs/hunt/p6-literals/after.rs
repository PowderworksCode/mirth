#[inline] pub fn a() -> &'static str { "literal" }
#[inline] pub fn b() -> &'static str { let s = "literal"; s }
