//! Example: `SerializeSeq` is a state machine, and its method receivers are
//! what make the protocol a compile-time rule rather than a documented one.
//!
//! The protocol is: `serialize_seq` hands you a value, you call
//! `serialize_element` on it any number of times, then `end` once. Two
//! signatures enforce it:
//!
//! - `fn serialize_element(&mut self, ...)` borrows, so it can run repeatedly.
//! - `fn end(self) -> Result<Self::Ok, Self::Error>` *consumes*, so nothing can
//!   follow it — and since `end` is the only method that produces `Self::Ok`,
//!   a `Serialize` impl cannot forget to call it and still return successfully.
//!
//! Neither rule needs a runtime state flag or a panic. The one thing the type
//! system does not check is that the number of elements matches the length hint
//! passed to `serialize_seq`, which is exactly why that hint is an `Option` and
//! why the docs call it a hint.
//!
//! The serializer below logs every transition, so the trace *is* the protocol.

use serde_core::ser::{self, Impossible, Serialize, SerializeSeq, Serializer};
use std::fmt::{self, Display};

#[derive(Debug)]
pub struct Error(String);

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

/// Generates the numeric and boolean methods, which differ only in their type.
macro_rules! scalars {
    ($($method:ident($ty:ty))*) => {
        $(
            fn $method(self, v: $ty) -> Result<(), Error> {
                self.note(format_args!("{}({v})", stringify!($method)));
                self.out.push_str(&v.to_string());
                Ok(())
            }
        )*
    };
}

/// Generates the compound entry points this format refuses, each returning the
/// `Impossible` type that satisfies the trait without being constructible.
macro_rules! unsupported {
    ($($method:ident($arg:ident: $ty:ty) -> $assoc:ident)*) => {
        $(
            fn $method(self, $arg: $ty) -> Result<Self::$assoc, Error> {
                let _ = $arg;
                Err(ser::Error::custom("this format handles scalars and sequences"))
            }
        )*
    };
}

/// The format's whole state: the JSON-ish text so far, a log of every call, and
/// the current nesting depth (used only to indent the log).
#[derive(Default)]
pub struct Trace {
    out: String,
    log: Vec<String>,
    depth: usize,
}

impl Trace {
    fn note(&mut self, msg: impl Display) {
        self.log
            .push(format!("{:width$}{msg}", "", width = self.depth * 2));
    }
}

/// The sequence state machine. It owns the serializer's state for as long as
/// the sequence is open, which is the borrow that stops anything else from
/// writing to `Trace` mid-sequence.
pub struct Seq<'a> {
    trace: &'a mut Trace,
    written: usize,
    announced: Option<usize>,
}

impl<'a> Serializer for &'a mut Trace {
    type Ok = ();
    type Error = Error;

    type SerializeSeq = Seq<'a>;
    type SerializeTuple = Impossible<(), Error>;
    type SerializeTupleStruct = Impossible<(), Error>;
    type SerializeTupleVariant = Impossible<(), Error>;
    type SerializeMap = Impossible<(), Error>;
    type SerializeStruct = Impossible<(), Error>;
    type SerializeStructVariant = Impossible<(), Error>;

    scalars! {
        serialize_bool(bool) serialize_char(char)
        serialize_i8(i8) serialize_i16(i16) serialize_i32(i32) serialize_i64(i64)
        serialize_u8(u8) serialize_u16(u16) serialize_u32(u32) serialize_u64(u64)
        serialize_f32(f32) serialize_f64(f64)
    }

    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.note(format_args!("serialize_str({v:?})"));
        self.out.push_str(&format!("{v:?}"));
        Ok(())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.note(format_args!("serialize_bytes({} bytes)", v.len()));
        self.out.push_str(&format!("\"<{} bytes>\"", v.len()));
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Error> {
        self.serialize_unit()
    }

    fn serialize_some<T>(self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<(), Error> {
        self.note("serialize_unit()");
        self.out.push_str("null");
        Ok(())
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<(), Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    /// Opens the state machine. The length is whatever the caller happens to
    /// know — `Some(n)` from a `Vec`, `None` from an iterator that cannot say.
    fn serialize_seq(self, len: Option<usize>) -> Result<Seq<'a>, Error> {
        self.note(format_args!("serialize_seq(len = {len:?})"));
        self.out.push('[');
        self.depth += 1;
        Ok(Seq {
            trace: self,
            written: 0,
            announced: len,
        })
    }

    unsupported! {
        serialize_tuple(_len: usize) -> SerializeTuple
        serialize_map(_len: Option<usize>) -> SerializeMap
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        Err(ser::Error::custom(
            "this format handles scalars and sequences",
        ))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Impossible<(), Error>, Error> {
        Err(ser::Error::custom(
            "this format handles scalars and sequences",
        ))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<(), Error>, Error> {
        Err(ser::Error::custom(
            "this format handles scalars and sequences",
        ))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Impossible<(), Error>, Error> {
        Err(ser::Error::custom(
            "this format handles scalars and sequences",
        ))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Impossible<(), Error>, Error> {
        Err(ser::Error::custom(
            "this format handles scalars and sequences",
        ))
    }
}

impl SerializeSeq for Seq<'_> {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize,
    {
        self.written += 1;
        self.trace
            .note(format_args!("serialize_element #{}", self.written));
        if self.written > 1 {
            self.trace.out.push_str(", ");
        }
        // A reborrow, not a move: the element gets its own serializer over the
        // same state, and `self` survives to take the next element.
        value.serialize(&mut *self.trace)
    }

    fn end(self) -> Result<(), Error> {
        self.trace.depth -= 1;
        self.trace.out.push(']');
        let verdict = match self.announced {
            Some(n) if n == self.written => format!("hint {n} matched"),
            Some(n) => format!("hint {n}, wrote {}", self.written),
            None => format!("no hint, wrote {}", self.written),
        };
        self.trace.note(format_args!("end() -> {verdict}"));
        Ok(())
    }
}

/// Serializes an iterator through `collect_seq`, whose default body asks the
/// iterator for a length hint. `map` over a slice keeps the exact size; `filter`
/// cannot, so the hint arrives as `None`.
pub struct Doubled<'a>(pub &'a [i32]);
pub struct Evens<'a>(pub &'a [i32]);

impl Serialize for Doubled<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(self.0.iter().map(|v| v * 2))
    }
}

impl Serialize for Evens<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(self.0.iter().filter(|v| *v % 2 == 0))
    }
}

fn render<T: ?Sized + Serialize>(label: &str, value: &T) -> String {
    let mut trace = Trace::default();
    let result = value.serialize(&mut trace);
    let mut out = format!("{label}\n");
    for line in &trace.log {
        out.push_str(&format!("  {line}\n"));
    }
    match result {
        Ok(()) => out.push_str(&format!("  = {}\n", trace.out)),
        Err(e) => out.push_str(&format!("  ! {e}\n")),
    }
    out
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();

    out.push_str("Every call the sequence protocol makes, in order.\n\n");

    out.push_str(&render("vec![10, 20, 30]", &vec![10, 20, 30]));
    out.push('\n');
    out.push_str(&render(
        "vec![vec![1, 2], vec![], vec![3]]",
        &vec![vec![1, 2], vec![], vec![3]],
    ));
    out.push('\n');

    let data = [1, 2, 3, 4, 5, 6];
    out.push_str(&render(
        "collect_seq(map)     — exact size",
        &Doubled(&data),
    ));
    out.push('\n');
    out.push_str(&render(
        "collect_seq(filter)  — size unknown",
        &Evens(&data),
    ));

    out.push_str(
        "\nBoth reached the same serialize_seq. Only the hint differs, because\n\
         iterator_len_hint returns Some only when the iterator's lower and upper\n\
         bounds agree — filter cannot promise that, map over a slice can.\n",
    );

    out
}
