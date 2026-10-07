//! Iterators, closures, patterns, control flow, collections, smart pointers.

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// A boxed closure pipeline.
pub struct Pipeline<T> {
    stages: Vec<Box<dyn Fn(T) -> T + Send + Sync>>,
}

impl<T> core::fmt::Debug for Pipeline<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Pipeline({} stages)", self.stages.len())
    }
}

impl<T: 'static> Pipeline<T> {
    pub fn new() -> Self {
        Pipeline { stages: Vec::new() }
    }

    pub fn then(mut self, f: impl Fn(T) -> T + Send + Sync + 'static) -> Self {
        self.stages.push(Box::new(f));
        self
    }

    pub fn run(&self, input: T) -> T {
        self.stages.iter().fold(input, |acc, f| f(acc))
    }
}

impl<T: 'static> Default for Pipeline<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Patterns: slices, ranges, bindings, guards, nested enums.
pub fn classify(values: &[i32]) -> &'static str {
    match values {
        [] => "empty",
        [x] if *x < 0 => "one negative",
        [_] => "one",
        [first, .., last] if first == last => "same ends",
        [0..=9, rest @ ..] if rest.len() > 2 => "small head, long",
        [_, _] => "pair",
        _ => "other",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Token {
    Num(i64),
    Word(String),
    Op(char),
    Group(Vec<Token>),
}

pub fn eval(tokens: &[Token]) -> Option<i64> {
    let mut stack: Vec<i64> = Vec::new();
    for token in tokens {
        match token {
            Token::Num(n) => stack.push(*n),
            Token::Op(op @ ('+' | '-' | '*')) => {
                let (Some(b), Some(a)) = (stack.pop(), stack.pop()) else {
                    return None;
                };
                stack.push(match op {
                    '+' => a + b,
                    '-' => a - b,
                    _ => a * b,
                });
            }
            Token::Group(inner) => stack.push(eval(inner)?),
            Token::Word(w) if w == "dup" => {
                let top = *stack.last()?;
                stack.push(top);
            }
            Token::Word(_) | Token::Op(_) => return None,
        }
    }
    stack.pop()
}

/// Labelled breaks, loops with values, `if let` chains.
pub fn find_pair(grid: &[Vec<i32>], target: i32) -> Option<(usize, usize)> {
    let found = 'outer: {
        for (i, row) in grid.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                if v == target {
                    break 'outer Some((i, j));
                }
            }
        }
        None
    };
    if let Some((i, j)) = found
        && i <= j
    {
        return Some((i, j));
    }
    found
}

pub fn collatz(mut n: u64) -> u32 {
    let mut steps = 0;
    loop {
        if n == 1 {
            break steps;
        }
        n = if n % 2 == 0 { n / 2 } else { 3 * n + 1 };
        steps += 1;
    }
}

/// Iterator adaptors and a custom iterator.
#[derive(Debug)]
pub struct Fib {
    a: u64,
    b: u64,
}

impl Iterator for Fib {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        let r = self.a;
        self.a = self.b;
        self.b += r;
        Some(r)
    }
}

pub fn fib() -> Fib {
    Fib { a: 0, b: 1 }
}

pub fn stats(words: &str) -> BTreeMap<usize, Vec<&str>> {
    let mut by_len: BTreeMap<usize, Vec<&str>> = BTreeMap::new();
    for w in words.split_whitespace().filter(|w| !w.is_empty()) {
        by_len.entry(w.len()).or_default().push(w);
    }
    by_len.values_mut().for_each(|v| v.sort_unstable());
    by_len
}

pub fn histogram(values: impl IntoIterator<Item = u8>) -> HashMap<u8, usize> {
    let mut h = HashMap::new();
    for v in values {
        *h.entry(v).or_insert(0) += 1;
    }
    h
}

pub fn unique_sorted<T: Ord + Clone>(items: &[T]) -> Vec<T> {
    items.iter().cloned().collect::<BTreeSet<_>>().into_iter().collect()
}

pub fn rotate<T>(mut q: VecDeque<T>, n: usize) -> VecDeque<T> {
    q.rotate_left(n % q.len().max(1));
    q
}

/// Cow, Rc/RefCell, Arc/Mutex and threads.
pub fn normalize(s: &str) -> Cow<'_, str> {
    if s.chars().any(char::is_uppercase) { Cow::Owned(s.to_lowercase()) } else { Cow::Borrowed(s) }
}

#[derive(Debug, Default)]
pub struct Node {
    pub value: i32,
    pub children: Vec<Rc<RefCell<Node>>>,
}

pub fn tree_sum(node: &Rc<RefCell<Node>>) -> i32 {
    let n = node.borrow();
    n.value + n.children.iter().map(tree_sum).sum::<i32>()
}

pub fn parallel_sum(chunks: Vec<Vec<u64>>) -> u64 {
    let total = Arc::new(Mutex::new(0));
    std::thread::scope(|s| {
        for chunk in &chunks {
            let total = Arc::clone(&total);
            s.spawn(move || *total.lock().unwrap() += chunk.iter().sum::<u64>());
        }
    });
    *total.lock().unwrap()
}

/// Closures capturing by move, by ref and by mut ref; returning closures.
pub fn counter() -> impl FnMut() -> u32 {
    let mut n = 0;
    move || {
        n += 1;
        n
    }
}

pub fn compose<A, B, C>(f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> impl Fn(A) -> C {
    move |x| g(f(x))
}

/// Sorting with closures and keys.
pub fn sort_people(people: &mut [(String, u32)]) {
    people.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
}
