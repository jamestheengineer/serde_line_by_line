//! Example: `Error::custom` — the only channel a data structure has for
//! reporting a problem to a format it has never met.
//!
//! `Ascii` refuses to serialize a string containing non-ASCII characters. It
//! cannot name a concrete error type: it is generic over `S: Serializer`, so
//! all it knows is that `S::Error` implements `ser::Error` and therefore has a
//! `custom` constructor accepting anything that implements `Display`.
//!
//! The surprise is in that constructor's signature. `fn custom<T>(msg: T) -> Self`
//! takes no `self` and no serializer, so an error type cannot look at the
//! format to discover *where* the failure happened. Anything richer than the
//! message has to be attached by the format afterwards — which is what
//! `Located::at` does below, and what serde_json does when it adds line and
//! column.

use serde_core::ser::{self, Impossible, Serialize, SerializeSeq, Serializer};
use std::fmt::{self, Display};

/// A `&str` that serializes only if it is pure ASCII.
///
/// Modeled on the `Path` example in serde_core's own `Error::custom` docs: the
/// data structure discovers the problem, the format owns the error type.
pub struct Ascii<'a>(pub &'a str);

impl Serialize for Ascii<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0.char_indices().find(|(_, c)| !c.is_ascii()) {
            // `format_args!` allocates nothing — `custom` takes any `Display`,
            // not a `String`. Lowercase and no trailing period, which is the
            // style the trait's docs ask for.
            Some((i, c)) => Err(ser::Error::custom(format_args!(
                "non-ASCII character {c:?} at byte {i}"
            ))),
            None => serializer.serialize_str(self.0),
        }
    }
}

/// The first format's error: the message, and nothing else.
#[derive(Debug)]
pub struct Plain(String);

/// The second format's error: the message plus how far the format had got.
///
/// `custom` cannot fill in `after`, so it starts as `None` and the format
/// supplies it once the failure has propagated back out.
#[derive(Debug)]
pub struct Located {
    msg: String,
    after: Option<usize>,
}

impl Located {
    fn at(mut self, values: usize) -> Self {
        self.after = Some(values);
        self
    }
}

impl Display for Plain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Display for Located {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.after {
            Some(n) => write!(f, "at value #{}: {}", n + 1, self.msg),
            None => f.write_str(&self.msg),
        }
    }
}

// Required by the supertrait. Under the `std` feature the trait is declared as
// `Error: Sized + StdError`, so every format's error type is usable anywhere a
// `Box<dyn std::error::Error>` is expected — demonstrated at the end of `run`.
impl std::error::Error for Plain {}
impl std::error::Error for Located {}

impl ser::Error for Plain {
    fn custom<T: Display>(msg: T) -> Self {
        Plain(msg.to_string())
    }
}

impl ser::Error for Located {
    fn custom<T: Display>(msg: T) -> Self {
        Located {
            msg: msg.to_string(),
            after: None,
        }
    }
}

/// Shared state for both formats: the text written so far, and a count of the
/// scalars that reached it.
#[derive(Default)]
pub struct Session {
    out: String,
    values: usize,
}

fn emit(session: &mut Session, text: &str) {
    if session.values > 0 {
        session.out.push(' ');
    }
    session.out.push_str(text);
    session.values += 1;
}

/// Writes scalars, errors with `Plain`.
pub struct Plainly<'a>(&'a mut Session);

/// Writes the same scalars, errors with `Located`.
pub struct Tracked<'a>(&'a mut Session);

/// Generates a whole space-separated scalar-and-sequence format. The two
/// invocations differ only in their error type, which is the entire point: the
/// `Serialize` impl above compiles against both without knowing either.
macro_rules! text_format {
    ($name:ident, $err:ty) => {
        impl<'a> Serializer for $name<'a> {
            type Ok = ();
            type Error = $err;

            type SerializeSeq = Self;
            type SerializeTuple = Impossible<(), $err>;
            type SerializeTupleStruct = Impossible<(), $err>;
            type SerializeTupleVariant = Impossible<(), $err>;
            type SerializeMap = Impossible<(), $err>;
            type SerializeStruct = Impossible<(), $err>;
            type SerializeStructVariant = Impossible<(), $err>;

            text_format!(@scalars $err;
                serialize_bool(bool) serialize_char(char)
                serialize_i8(i8) serialize_i16(i16) serialize_i32(i32) serialize_i64(i64)
                serialize_u8(u8) serialize_u16(u16) serialize_u32(u32) serialize_u64(u64)
                serialize_f32(f32) serialize_f64(f64));

            fn serialize_str(self, v: &str) -> Result<(), $err> {
                emit(self.0, &format!("{v:?}"));
                Ok(())
            }
            fn serialize_bytes(self, v: &[u8]) -> Result<(), $err> {
                emit(self.0, &format!("<{} bytes>", v.len()));
                Ok(())
            }
            fn serialize_none(self) -> Result<(), $err> {
                self.serialize_unit()
            }
            fn serialize_some<T>(self, value: &T) -> Result<(), $err>
            where
                T: ?Sized + Serialize,
            {
                value.serialize(self)
            }
            fn serialize_unit(self) -> Result<(), $err> {
                emit(self.0, "null");
                Ok(())
            }
            fn serialize_unit_struct(self, _name: &'static str) -> Result<(), $err> {
                self.serialize_unit()
            }
            fn serialize_unit_variant(
                self,
                _name: &'static str,
                _index: u32,
                variant: &'static str,
            ) -> Result<(), $err> {
                self.serialize_str(variant)
            }
            fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<(), $err>
            where
                T: ?Sized + Serialize,
            {
                value.serialize(self)
            }
            fn serialize_seq(self, _len: Option<usize>) -> Result<Self, $err> {
                Ok(self)
            }

            fn serialize_newtype_variant<T>(
                self,
                _name: &'static str,
                _index: u32,
                _variant: &'static str,
                _value: &T,
            ) -> Result<(), $err>
            where
                T: ?Sized + Serialize,
            {
                Err(ser::Error::custom("this format holds scalars and sequences only"))
            }

            fn serialize_tuple(self, _len: usize) -> Result<Impossible<(), $err>, $err> {
                Err(ser::Error::custom("this format holds scalars and sequences only"))
            }
            fn serialize_tuple_struct(
                self,
                _name: &'static str,
                _len: usize,
            ) -> Result<Impossible<(), $err>, $err> {
                Err(ser::Error::custom("this format holds scalars and sequences only"))
            }
            fn serialize_tuple_variant(
                self,
                _name: &'static str,
                _index: u32,
                _variant: &'static str,
                _len: usize,
            ) -> Result<Impossible<(), $err>, $err> {
                Err(ser::Error::custom("this format holds scalars and sequences only"))
            }
            fn serialize_map(self, _len: Option<usize>) -> Result<Impossible<(), $err>, $err> {
                Err(ser::Error::custom("this format holds scalars and sequences only"))
            }
            fn serialize_struct(
                self,
                _name: &'static str,
                _len: usize,
            ) -> Result<Impossible<(), $err>, $err> {
                Err(ser::Error::custom("this format holds scalars and sequences only"))
            }
            fn serialize_struct_variant(
                self,
                _name: &'static str,
                _index: u32,
                _variant: &'static str,
                _len: usize,
            ) -> Result<Impossible<(), $err>, $err> {
                Err(ser::Error::custom("this format holds scalars and sequences only"))
            }
        }

        impl<'a> SerializeSeq for $name<'a> {
            type Ok = ();
            type Error = $err;

            fn serialize_element<T>(&mut self, value: &T) -> Result<(), $err>
            where
                T: ?Sized + Serialize,
            {
                // Reborrow: each element gets a fresh serializer over the same
                // session, which is how the count keeps rising across elements.
                value.serialize($name(&mut *self.0))
            }

            fn end(self) -> Result<(), $err> {
                Ok(())
            }
        }
    };

    (@scalars $err:ty; $($method:ident($ty:ty))*) => {
        $(
            fn $method(self, v: $ty) -> Result<(), $err> {
                emit(self.0, &v.to_string());
                Ok(())
            }
        )*
    };
}

text_format!(Plainly, Plain);
text_format!(Tracked, Located);

/// Renders with the format whose error carries only a message.
pub fn plainly<T: ?Sized + Serialize>(value: &T) -> Result<String, Plain> {
    let mut session = Session::default();
    value.serialize(Plainly(&mut session))?;
    Ok(session.out)
}

/// Renders with the format whose error carries the message *and* the position.
///
/// The position is attached here, not in `custom`. This function is the first
/// place in the call chain that can see both the failure and the serializer's
/// state at the same time.
pub fn tracked<T: ?Sized + Serialize>(value: &T) -> Result<String, Located> {
    let mut session = Session::default();
    match value.serialize(Tracked(&mut session)) {
        Ok(()) => Ok(session.out),
        Err(e) => Err(e.at(session.values)),
    }
}

/// A `Display` type that is not a string, to show what `custom` accepts.
struct Utf8Bytes<'a>(&'a str);

impl Display for Utf8Bytes<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} bytes for {} chars",
            self.0.len(),
            self.0.chars().count()
        )
    }
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();

    out.push_str("One Serialize impl, two formats, two error types.\n\n");

    for words in [
        ["ok", "fine", "good"],
        ["ok", "fine", "h\u{e9}llo"],
        ["caf\u{e9}", "fine", "good"],
    ] {
        let values: Vec<Ascii> = words.iter().map(|w| Ascii(w)).collect();
        out.push_str(&format!("input     {words:?}\n"));
        match plainly(&values) {
            Ok(text) => out.push_str(&format!("  Plain     ok: {text}\n")),
            Err(e) => out.push_str(&format!("  Plain     error: {e}\n")),
        }
        match tracked(&values) {
            Ok(text) => out.push_str(&format!("  Located   ok: {text}\n")),
            Err(e) => out.push_str(&format!("  Located   error: {e}\n")),
        }
        out.push('\n');
    }

    out.push_str("Plain cannot say where it happened. Located can, because the\n");
    out.push_str("format attached the count after custom had already returned.\n\n");

    out.push_str("custom takes anything that implements Display:\n");
    let messages: [Plain; 4] = [
        ser::Error::custom("a &str"),
        ser::Error::custom('x'),
        ser::Error::custom(format_args!("format_args!, {} allocations", 0)),
        ser::Error::custom(Utf8Bytes("h\u{e9}llo")),
    ];
    for m in &messages {
        out.push_str(&format!("  {m}\n"));
    }

    // The supertrait at work: `Error: Sized + StdError` under the std feature.
    let boxed: Box<dyn std::error::Error> = Box::new(plainly(&Ascii("caf\u{e9}")).unwrap_err());
    out.push_str(&format!(
        "\nAnd because Error: StdError under std, it boxes: {boxed}\n"
    ));

    out
}
