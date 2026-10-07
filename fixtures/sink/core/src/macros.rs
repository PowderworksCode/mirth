//! Declarative macros: recursive, exported, with hygiene.

/// Sums its arguments at compile time.
#[macro_export]
macro_rules! sum {
    () => { 0 };
    ($head:expr $(, $tail:expr)* $(,)?) => { $head + $crate::sum!($($tail),*) };
}

/// Builds a `HashMap` from `key => value` pairs.
#[macro_export]
macro_rules! map {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut map = ::std::collections::HashMap::new();
        $(map.insert($key, $value);)*
        map
    }};
}

/// Defines a newtype with `Deref`, used within this crate only.
macro_rules! newtype {
    ($(#[$attr:meta])* $vis:vis $name:ident($inner:ty)) => {
        $(#[$attr])*
        #[derive(Debug, Clone, PartialEq)]
        $vis struct $name(pub $inner);

        impl ::core::ops::Deref for $name {
            type Target = $inner;
            fn deref(&self) -> &$inner {
                &self.0
            }
        }
    };
}

newtype!(
    /// Metres, as a newtype.
    pub Metres(f64)
);

/// Hygiene: the `x` inside does not see the caller's `x`.
#[macro_export]
macro_rules! shadow {
    ($e:expr) => {{
        let x = 10;
        $e + x
    }};
}
