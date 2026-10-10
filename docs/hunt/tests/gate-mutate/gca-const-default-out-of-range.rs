#![feature(gca_macroless_args)]
#![feature(generic_const_exprs)]
#![feature(gca_min_const_items)]

fn pass_enum<const N: usize, const M: usize = const { N }> {
    pass_enum::<{ gca!(None) }>
}
