//! Example: which conversion each numeric `Deserialize` impl applies, and
//! where it refuses.
//!
//! `de/impls.rs` builds the sixteen numeric impls out of one `impl_deserialize_num!`
//! and six one-method macros — `num_self!`, `num_as_self!`,
//! `num_as_copysign_self!`, `int_to_int!`, `int_to_uint!`, `uint_to_self!`.
//! Reading the invocation list tells you which macro was chosen for which
//! source type. It does not tell you what that choice *does* to a value at the
//! edges, which is the part that decides whether real input is accepted.
//!
//! `Feed` is a self-describing "format" holding exactly one value. Every
//! `deserialize_*` hint forwards to `deserialize_any`, which calls the single
//! `visit_*` method matching what it holds. So `u8::deserialize(Feed::I64(300))`
//! runs precisely the arm that `int_to_uint!(i64:visit_i64)` generated for `u8`,
//! and nothing else.
//!
//! Four things this is here to prove:
//!
//! 1. Widening never fails and narrowing is range-checked at run time, not
//!    silently truncated — the check lives in the macro, not in the format.
//! 2. `NonZero*` rejects zero with a message naming what was expected, and
//!    `Saturating<T>` clamps instead of failing. Both come from extra arms of
//!    the same six macros.
//! 3. Float conversions are the exception: they are *not* range-checked. They
//!    saturate to infinity and lose precision without complaint.
//! 4. None of these visitors has a `visit_str`. A number in quotes is a type
//!    error, not a parse.

use serde_core::de::{Deserialize, Deserializer, Visitor};
use serde_core::forward_to_deserialize_any;
use std::fmt::{self, Display};
use std::num::{NonZeroI8, NonZeroU8, NonZeroU32, Saturating};

/// The one value a `Feed` holds, tagged with the visit method it will call.
#[derive(Clone, Copy)]
pub enum Feed {
    I8(i8),
    I64(i64),
    U8(u8),
    U64(u64),
    I128(i128),
    U128(u128),
    F32(f32),
    F64(f64),
    Str(&'static str),
}

impl Display for Feed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Feed::I8(v) => write!(f, "visit_i8({v})"),
            Feed::I64(v) => write!(f, "visit_i64({v})"),
            Feed::U8(v) => write!(f, "visit_u8({v})"),
            Feed::U64(v) => write!(f, "visit_u64({v})"),
            Feed::I128(v) => write!(f, "visit_i128({v})"),
            Feed::U128(v) => write!(f, "visit_u128({v})"),
            Feed::F32(v) => write!(f, "visit_f32({v:?})"),
            Feed::F64(v) => write!(f, "visit_f64({v:?})"),
            Feed::Str(v) => write!(f, "visit_str({v:?})"),
        }
    }
}

#[derive(Debug)]
pub struct Error(String);

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl serde_core::de::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

impl<'de> Deserializer<'de> for Feed {
    type Error = Error;

    /// A self-describing format: the value decides the method, the hint is
    /// ignored. That is what makes the visitor's own conversion table the only
    /// thing under test here.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self {
            Feed::I8(v) => visitor.visit_i8(v),
            Feed::I64(v) => visitor.visit_i64(v),
            Feed::U8(v) => visitor.visit_u8(v),
            Feed::U64(v) => visitor.visit_u64(v),
            Feed::I128(v) => visitor.visit_i128(v),
            Feed::U128(v) => visitor.visit_u128(v),
            Feed::F32(v) => visitor.visit_f32(v),
            Feed::F64(v) => visitor.visit_f64(v),
            Feed::Str(v) => visitor.visit_str(v),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();

    let mut section = |title: &str, note: &str, rows: Vec<(String, String, String)>| {
        out.push_str(title);
        out.push('\n');
        out.push_str(&format!("{}\n", "-".repeat(title.len())));
        for (target, source, result) in rows {
            out.push_str(&format!("{target:<18} {source:<22} {result}\n"));
        }
        out.push('\n');
        out.push_str(note);
        out.push_str("\n\n");
    };

    macro_rules! row {
        ($target:ty, $feed:expr) => {{
            let feed = $feed;
            let result = match <$target>::deserialize(feed) {
                Ok(v) => format!("{v:?}"),
                Err(e) => format!("Err: {e}"),
            };
            (stringify!($target).to_string(), feed.to_string(), result)
        }};
    }

    section(
        "num_as_self! and num_self! — the conversions that cannot fail",
        "The target is at least as wide as the source, so the macro emits a bare\n\
         `v as Self::Value` with no check. num_self! is the same arm with the\n\
         cast removed, for the one source type that is already the target.",
        vec![
            row!(i64, Feed::I8(-8)),
            row!(i64, Feed::I64(-64)),
            row!(u64, Feed::U8(8)),
            row!(i128, Feed::U64(u64::MAX)),
        ],
    );

    section(
        "int_to_int! and uint_to_self! — narrowing, checked at run time",
        "Both widen to i64/u64 first and then `try_from` into the target, so one\n\
         macro arm covers every source width. Out of range is an error, never a\n\
         truncation: 300 does not become 44.",
        vec![
            row!(i8, Feed::I64(127)),
            row!(i8, Feed::I64(128)),
            row!(u8, Feed::U64(255)),
            row!(u8, Feed::U64(256)),
        ],
    );

    section(
        "int_to_uint! — the sign check that comes first",
        "Signed source, unsigned target. The `if 0 <= v` guard runs before the\n\
         `try_from`, which is why -1 reports the value it saw rather than a\n\
         conversion failure.",
        vec![
            row!(u8, Feed::I64(300)),
            row!(u8, Feed::I64(-1)),
            row!(u8, Feed::I8(-1)),
        ],
    );

    section(
        "The nonzero arms — same conversion, one extra rejection",
        "`impl_deserialize_num!`'s first arm generates a second impl for the\n\
         matching NonZero type out of the same macro list, adding a\n\
         `Self::Value::new(v)` step. Note the message: the expectation comes\n\
         from the visitor's `expecting`, so it names the primitive.",
        vec![
            row!(NonZeroU32, Feed::U64(7)),
            row!(NonZeroU32, Feed::U64(0)),
            row!(NonZeroU8, Feed::I64(-3)),
            row!(NonZeroI8, Feed::I64(200)),
        ],
    );

    section(
        "The saturating arms — clamping instead of failing",
        "The third arm of each macro. Same input as the checked rows above, and\n\
         the only impls here that answer an out-of-range number with a number.",
        vec![
            row!(Saturating<u8>, Feed::I64(300)),
            row!(Saturating<u8>, Feed::I64(-1)),
            row!(Saturating<i8>, Feed::I64(-999)),
        ],
    );

    section(
        "num_128! — the range check that cannot borrow i64/u64",
        "128-bit targets cannot widen to i64 first, so this macro compares in\n\
         i128/u128 directly. The error also reads differently: Unexpected has no\n\
         128-bit variant, so the macro passes Unexpected::Other with the type\n\
         name and you get the type instead of the value.",
        vec![
            row!(i128, Feed::U128(u128::MAX)),
            row!(u128, Feed::I128(-1)),
            row!(u128, Feed::U128(u128::MAX)),
        ],
    );

    section(
        "The float arms — num_as_copysign_self!, and no range check anywhere",
        "The float arms have no guard. Out of range saturates to infinity,\n\
         precision is dropped silently, and the only thing the macro takes care\n\
         to preserve is the sign of NaN — `as` alone is allowed to lose it.",
        vec![
            row!(f32, Feed::F64(1e300)),
            row!(f32, Feed::F64(16777217.0)),
            row!(f64, Feed::U64(u64::MAX)),
            (
                "f32".to_string(),
                "visit_f64(-NaN)".to_string(),
                match f32::deserialize(Feed::F64(f64::NAN.copysign(-1.0))) {
                    Ok(v) => format!("NaN, sign_negative = {}", v.is_sign_negative()),
                    Err(e) => format!("Err: {e}"),
                },
            ),
        ],
    );

    section(
        "The method that is missing: visit_str",
        "None of the sixteen numeric visitors overrides visit_str, so the\n\
         default from the Visitor trait runs and reports a type error. serde\n\
         never parses a number out of a string on its own; a format that wants\n\
         that has to call visit_u64 itself.",
        vec![row!(u8, Feed::Str("7")), row!(f64, Feed::Str("1.5"))],
    );

    out
}
