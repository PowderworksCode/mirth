//! Runs everything and checks the results, so a miscompile shows as a failure.

use std::collections::VecDeque;

use sink_core::algo::{self, Token};
use sink_core::asyncs::{self, Static};
use sink_core::consts::{self, HasId, Matrix};
use sink_core::memory;
use sink_core::shapes::{self, Container, V2};
use sink_core::{Circle, Describe, Shape, Square, map};
use sink_mid::{Colour, Polygon};

fn main() {
    let mut checks: Vec<(&str, bool)> = Vec::new();
    let mut check = |name, ok| checks.push((name, ok));

    let sq = Square { side: 2.0 };
    let ci = Circle { radius: 1.0 };
    let poly = Polygon { sides: 6, length: 1.0, label: "hexagon" };
    let shapes: Vec<&dyn Shape<Unit = f64>> = vec![&sq, &ci, &poly];
    check("total area", (shapes::total_area(shapes.iter().copied()) - 9.739).abs() < 0.01);
    check("upcast", shapes::as_area(&sq).area() == 4.0);
    check("largest", sink_mid::largest(&[&sq, &sq]) == Some(4.0));
    check("perimeter", poly.perimeter() == 6.0 && sq.perimeter() == 8.0);
    check("derive", poly.describe() == "Polygon with 3 fields, tagged polygon");
    check("derive enum", Colour::NAME == "Colour" && sink_mid::colours().len() == 3);
    check("operators", (V2 { x: 1.0, y: 2.0 } + V2::from((1.0, 1.0))) * 2.0 == V2 { x: 4.0, y: 6.0 });
    check("index", V2 { x: 3.0, y: 4.0 }[1] == 4.0);
    check("gat", vec![1, 2, 3].first_matching(|x| **x > 1) == Some(&2));
    check("rpitit", sink_mid::labels(&[sq]) == vec!["square 2".to_string()]);
    check("statics", sink_core::extras::WORDS[1] == "beta" && sink_core::extras::NESTED[2] == [1, 2, 3]);
    check("include", sink_core::extras::DATA.trim() == "included text" && sink_core::extras::BYTES.len() == 14);
    check("no_mangle", sink_core::extras::sink_extras_add(2, 3) == 5);
    check("path mod", sink_core::extras::inner::twice(4) == 8 && sink_core::find(2) == Some("two"));
    check("any", sink_core::extras::kind(&5u32) == "u32" && sink_core::extras::kind(&"x") == "str");
    check("inline across crates", sink_mid::first_two(&[1, 2, 3]) == Some(&[1, 2][..]) && sink_mid::words() == 3 && sink_mid::hygienic(4) == 5);
    check("hrtb", shapes::apply_to_all(&["ab".into(), "cde".into()], |s| s.trim()) == vec![2, 3]);

    check("pipeline", algo::Pipeline::new().then(|x: i32| x + 1).then(|x| x * 10).run(1) == 20);
    check("classify", algo::classify(&[1, 5, 6, 7]) == "small head, long" && algo::classify(&[3, 9, 3]) == "same ends");
    let program = [Token::Num(2), Token::Group(vec![Token::Num(3), Token::Word("dup".into()), Token::Op('*')]), Token::Op('+')];
    check("eval", algo::eval(&program) == Some(11));
    check("labelled break", algo::find_pair(&[vec![1, 2], vec![3, 4]], 4) == Some((1, 1)));
    check("loop value", algo::collatz(27) == 111);
    check("fib", algo::fib().skip(10).next() == Some(55));
    check("btree", algo::stats("a bb cc d").get(&2).map(Vec::len) == Some(2));
    check("histogram", algo::histogram([1, 1, 2]).get(&1) == Some(&2));
    check("unique", algo::unique_sorted(&[3, 1, 3, 2]) == vec![1, 2, 3]);
    check("deque", algo::rotate(VecDeque::from(vec![1, 2, 3]), 1) == VecDeque::from(vec![2, 3, 1]));
    check("cow", algo::normalize("Hi") == "hi" && matches!(algo::normalize("hi"), std::borrow::Cow::Borrowed(_)));
    let leaf = std::rc::Rc::new(std::cell::RefCell::new(algo::Node { value: 2, children: vec![] }));
    let root = std::rc::Rc::new(std::cell::RefCell::new(algo::Node { value: 1, children: vec![leaf.clone(), leaf] }));
    check("rc tree", algo::tree_sum(&root) == 5);
    check("threads", algo::parallel_sum(vec![vec![1, 2], vec![3], vec![4, 5, 6]]) == 21);
    let mut c = algo::counter();
    c();
    check("closure state", c() == 2);
    check("compose", algo::compose(|x: u8| x as u32 + 1, |y| y * 3)(4) == 15);
    let mut people = vec![("b".to_string(), 3), ("a".to_string(), 3), ("c".to_string(), 9)];
    algo::sort_people(&mut people);
    check("sort", people[0].0 == "c" && people[1].0 == "a");

    check("async", asyncs::block_on(asyncs::add_later(2, 3)) == 5);
    let store = Static(&[("k", "v"), ("x", "y")]);
    check("afit", asyncs::block_on(sink_mid::lookup(&store, &["x", "missing", "k"])) == vec!["y", "v"]);
    check("async closure", asyncs::block_on(asyncs::call_twice(asyncs::greeter("sink".into()))) == "hello, sink; bye, sink");
    check("boxed future", asyncs::block_on(asyncs::boxed_future(21)) == 42);

    let m = Matrix::<2, 3> { cells: [[1, 2, 3], [4, 5, 6]] };
    check("const generics", m.mul(&m.transpose()).cells == [[14, 32], [32, 77]]);
    check("dylib", sink_dy::spin(&m).cells[2] == [3, 6] && sink_dy::twice(1) == 16);
    check("const fn", consts::FACT_10 == 3_628_800 && consts::TABLE[5] == 120);
    check("identity", Matrix::<3, 3>::identity_like().cells[2][2] == 1 && Matrix::<2, 5>::SIZE == 10);
    check("first_n", consts::first_n::<2>(&[9, 8, 7]) == Some([9, 8]));
    check("inline const", consts::inline_const().iter().all(Vec::is_empty));
    check("assoc const", consts::B.id() == 2);
    check("literals", consts::BYTES.len() == 7 && consts::C_STR.to_bytes().len() == 8 && consts::CHAR.len_utf8() == 4);

    check("union", memory::float_bits(1.0) == 0x3ff0_0000_0000_0000);
    check("repr", memory::Level::High as u8 == 9);
    {
        let _a = memory::Loud(2);
        let _b = memory::Loud(3);
    }
    check("drop", memory::DROPS.load(std::sync::atomic::Ordering::SeqCst) == 5);
    memory::count_call();
    check("thread local", memory::count_call() == 2);
    let mut v = vec![1, 2, 3, 4, 5];
    memory::reverse_in_place(&mut v);
    check("raw pointers", v == [5, 4, 3, 2, 1]);
    check("maybeuninit", memory::init_array() == [0, 7, 14, 21]);
    check("manuallydrop", memory::keep_alive("four".into()) == 4);
    check("ffi", memory::sink_core_add(2, 3) == 5 && memory::c_abs(-4) == 4);
    check("track caller", memory::caller_line() == line!());
    check("inline attrs", memory::never_inline(memory::always_inline(1)) == 0x1000_0000);

    check("errors", sink_core::errors::sum_all(&["1", "2"]).ok() == Some(3));
    check("error kinds", sink_core::errors::parse_bounded("5000", 10).unwrap_err().to_string() == "5000 is over 10");
    check("try option", sink_core::errors::first_char_upper("abc") == Some('A'));

    check("proc macro attr", sink_mid::traced_square(3) == 9 && sink_core::TRACE.load(std::sync::atomic::Ordering::Relaxed) == 1);
    check("proc macro fn", sink_mid::squared_table() == [1, 4, 9, 16]);
    check("macro_rules", sink_mid::macro_sum() == 15 && sink_mid::shadowing() == 11);
    check("map macro", map! { "a" => 1, "b" => 2 }.len() == 2);
    check("build script", sink_core::generated() && sink_core::version() == (7, "sink"));
    check("feature", sink_core::extra() == "extra");
    check("visibility", sink_mid::nested::shown() == 16);
    check("wrapper", sink_mid::Wrapper::<u8, 2>::filled(1).describe() == "2 items: [1, 1]");

    let failed: Vec<_> = checks.iter().filter(|(_, ok)| !ok).map(|(name, _)| *name).collect();
    if failed.is_empty() {
        println!("{} checks passed", checks.len());
    } else {
        println!("failed: {failed:?}");
        std::process::exit(1);
    }
}
