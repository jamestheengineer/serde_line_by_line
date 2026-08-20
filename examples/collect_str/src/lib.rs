//! Example: `collect_str` — serializing a value as a string it never builds.
//!
//! A type whose textual form is computed, not stored — a version number, a
//! timestamp, a path — has nothing to hand `serialize_str`, which wants a
//! `&str` that already exists. `collect_str` takes a `Display` instead and lets
//! the *format* decide what that costs.
//!
//! The default body is one line:
//!
//! ```text
//! self.serialize_str(&value.to_string())
//! ```
//!
//! It allocates a `String`, hands out a borrow of it, and drops it. Every
//! format gets that for free and most are happy with it. A format that writes
//! to a buffer it already owns can override `collect_str` and format directly
//! into that buffer, and then no intermediate string exists at any point. Both
//! formats below serialize identical values; they differ by that one method.
//!
//! The `#[cfg]` on the default matters too: without `std` or `alloc` there is
//! nothing to allocate into, so `collect_str` is declared with no body at all
//! and every `no_std` format must write this method itself.

use serde_core::ser::{self, Impossible, Serialize, Serializer};
use std::fmt::{self, Display, Write as _};

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

/// What each format records while it works.
#[derive(Default)]
pub struct Session {
    out: String,
    events: Vec<String>,
}

/// Uses the inherited `collect_str`.
pub struct Naive<'a>(&'a mut Session);

/// Overrides it.
pub struct Streaming<'a>(&'a mut Session);

/// A `fmt::Write` sink that appends to the format's own buffer and counts how
/// many pieces the `Display` impl produced. The count is the measurement: five
/// pieces means the value was written in five steps and never assembled
/// anywhere first.
struct Pieces<'a> {
    out: &'a mut String,
    pieces: usize,
}

impl fmt::Write for Pieces<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.pieces += 1;
        self.out.push_str(s);
        Ok(())
    }
}

/// Generates a string-only format. The second invocation adds one method.
macro_rules! string_format {
    ($name:ident, { $($extra:tt)* }) => {
        impl<'a> Serializer for $name<'a> {
            type Ok = ();
            type Error = Error;

            type SerializeSeq = Impossible<(), Error>;
            type SerializeTuple = Impossible<(), Error>;
            type SerializeTupleStruct = Impossible<(), Error>;
            type SerializeTupleVariant = Impossible<(), Error>;
            type SerializeMap = Impossible<(), Error>;
            type SerializeStruct = Impossible<(), Error>;
            type SerializeStructVariant = Impossible<(), Error>;

            fn serialize_str(self, v: &str) -> Result<(), Error> {
                // Whatever arrives here is a `&str` that already exists in
                // memory somewhere. When the caller was the default
                // `collect_str`, "somewhere" is a temporary String it built.
                self.0
                    .events
                    .push(format!("serialize_str(&str, {} bytes)", v.len()));
                self.0.out.push_str(v);
                Ok(())
            }

            $($extra)*

            string_format!(@scalars
                serialize_bool(bool) serialize_char(char)
                serialize_i8(i8) serialize_i16(i16) serialize_i32(i32) serialize_i64(i64)
                serialize_u8(u8) serialize_u16(u16) serialize_u32(u32) serialize_u64(u64)
                serialize_f32(f32) serialize_f64(f64));

            fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
                self.serialize_str(&format!("<{} bytes>", v.len()))
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
                self.serialize_str("null")
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
                Err(ser::Error::custom("this format writes strings only"))
            }

            string_format!(@compound
                serialize_seq(Option<usize>) -> SerializeSeq
                serialize_tuple(usize) -> SerializeTuple
                serialize_map(Option<usize>) -> SerializeMap);

            fn serialize_tuple_struct(
                self,
                _name: &'static str,
                _len: usize,
            ) -> Result<Impossible<(), Error>, Error> {
                Err(ser::Error::custom("this format writes strings only"))
            }
            fn serialize_tuple_variant(
                self,
                _name: &'static str,
                _index: u32,
                _variant: &'static str,
                _len: usize,
            ) -> Result<Impossible<(), Error>, Error> {
                Err(ser::Error::custom("this format writes strings only"))
            }
            fn serialize_struct(
                self,
                _name: &'static str,
                _len: usize,
            ) -> Result<Impossible<(), Error>, Error> {
                Err(ser::Error::custom("this format writes strings only"))
            }
            fn serialize_struct_variant(
                self,
                _name: &'static str,
                _index: u32,
                _variant: &'static str,
                _len: usize,
            ) -> Result<Impossible<(), Error>, Error> {
                Err(ser::Error::custom("this format writes strings only"))
            }
        }
    };

    (@scalars $($method:ident($ty:ty))*) => {
        $(
            fn $method(self, v: $ty) -> Result<(), Error> {
                self.serialize_str(&v.to_string())
            }
        )*
    };

    (@compound $($method:ident($ty:ty) -> $assoc:ident)*) => {
        $(
            fn $method(self, len: $ty) -> Result<Self::$assoc, Error> {
                let _ = len;
                Err(ser::Error::custom("this format writes strings only"))
            }
        )*
    };
}

string_format!(Naive, {});

string_format!(Streaming, {
    /// The whole difference between the two formats.
    ///
    /// `write!` drives the `Display` impl straight into the output buffer. The
    /// value is rendered exactly once, in pieces, and no `String` is created to
    /// hold it in between.
    fn collect_str<T>(self, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Display,
    {
        let start = self.0.out.len();
        let mut sink = Pieces {
            out: &mut self.0.out,
            pieces: 0,
        };
        write!(sink, "{value}").map_err(ser::Error::custom)?;
        let pieces = sink.pieces;
        let bytes = self.0.out.len() - start;
        let plural = if pieces == 1 { "piece" } else { "pieces" };
        self.0.events.push(format!(
            "collect_str(Display) -> {pieces} {plural}, {bytes} bytes, 0 allocated in between"
        ));
        Ok(())
    }
});

/// A version number: three integers, and a textual form that exists only while
/// it is being written.
pub struct Version(pub u16, pub u16, pub u16);

impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

impl Serialize for Version {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

/// `collect_str` takes `T: ?Sized`, so an unsized `str` goes in directly.
pub struct Borrowed<'a>(pub &'a str);

impl Serialize for Borrowed<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self.0)
    }
}

/// The pattern serde_core's own docs use for `DateTime`: `format_args!` builds
/// no string, it builds a recipe for one.
pub struct Instant {
    pub seconds: u64,
    pub nanos: u32,
}

impl Serialize for Instant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(&format_args!("{}.{:09}s", self.seconds, self.nanos))
    }
}

fn render<T: ?Sized + Serialize>(label: &str, value: &T) -> String {
    let mut naive = Session::default();
    let mut streaming = Session::default();
    value.serialize(Naive(&mut naive)).unwrap();
    value.serialize(Streaming(&mut streaming)).unwrap();

    let mut out = format!("{label}\n");
    out.push_str(&format!("  Naive      {}\n", naive.events.join("; ")));
    out.push_str(&format!("  Streaming  {}\n", streaming.events.join("; ")));
    assert_eq!(naive.out, streaming.out, "the formats must agree on output");
    out.push_str(&format!("  both wrote {}\n", naive.out));
    out
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();

    out.push_str("Two formats. One overrides collect_str, the other inherits it.\n\n");

    out.push_str(&render("Version(1, 2, 3)", &Version(1, 2, 3)));
    out.push('\n');
    out.push_str(&render("Version(10, 0, 12345)", &Version(10, 0, 12345)));
    out.push('\n');
    out.push_str(&render(
        "Borrowed(\"already a str\")",
        &Borrowed("already a str"),
    ));
    out.push('\n');
    out.push_str(&render(
        "Instant { seconds: 1431709260, nanos: 500 }",
        &Instant {
            seconds: 1_431_709_260,
            nanos: 500,
        },
    ));

    out.push_str(
        "\nNaive never sees a Display value: the default collect_str resolved it\n\
         to a String and passed a borrow of that String to serialize_str. The\n\
         piece counts show what Streaming saw instead — the Display impl writing\n\
         its parts one at a time, into the output buffer itself.\n\n\
         Borrowed is the case where the default costs a copy of a string that\n\
         already existed. Instant is the case where no string ever exists:\n\
         format_args! is a recipe, and only Streaming declines to cook it twice.\n",
    );

    out
}
