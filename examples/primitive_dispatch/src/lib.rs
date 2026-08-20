//! Example: what each of the sixteen `primitive_impl!` invocations dispatches to.
//!
//! `Trace` is a `Serializer` whose `Ok` type is the *call it received* rather
//! than any encoded output. Serializing a value therefore reports which
//! `serialize_*` method the value's `Serialize` impl chose, and with what
//! argument. That turns the macro expansion in `src/ser/impls.rs` into
//! something observable instead of something you have to take on trust.
//!
//! Two facts this is here to prove:
//!
//! 1. `isize` and `usize` do not have methods of their own. They widen to
//!    `serialize_i64` / `serialize_u64`, and the method name is the same
//!    whether this runs on a 64-bit host or as 32-bit WASM.
//! 2. `serialize_i128` and `serialize_u128` are the only two of the sixteen
//!    with a default body, and that body returns an error. `Trace` does not
//!    override them, so it behaves like a format that never opted in.

use serde_core::ser::{Error as _, Impossible, Serialize, Serializer};
use std::fmt::{self, Display};

/// The call a value made, captured instead of encoded.
pub struct Call {
    pub method: &'static str,
    pub arg: String,
}

#[derive(Clone, Copy)]
pub struct Trace;

#[derive(Debug)]
pub struct Error(String);

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl serde_core::ser::Error for Error {
    fn custom<T: Display>(msg: T) -> Self {
        Error(msg.to_string())
    }
}

/// Records the method name and its argument. Deliberately shaped like
/// `primitive_impl!` itself: one matcher, one small body, many expansions.
macro_rules! record {
    ($($method:ident($ty:ty)),* $(,)?) => {
        $(
            fn $method(self, v: $ty) -> Result<Call, Error> {
                Ok(Call { method: stringify!($method), arg: v.to_string() })
            }
        )*
    };
}

impl Serializer for Trace {
    type Ok = Call;
    type Error = Error;

    type SerializeSeq = Impossible<Call, Error>;
    type SerializeTuple = Impossible<Call, Error>;
    type SerializeTupleStruct = Impossible<Call, Error>;
    type SerializeTupleVariant = Impossible<Call, Error>;
    type SerializeMap = Impossible<Call, Error>;
    type SerializeStruct = Impossible<Call, Error>;
    type SerializeStructVariant = Impossible<Call, Error>;

    // The twelve required scalar methods the macro's invocations reach. Note
    // what is absent: there is no `serialize_isize` or `serialize_usize` to
    // implement, and `serialize_i128` / `serialize_u128` are left to their
    // erroring defaults on purpose.
    record! {
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char),
    }

    fn serialize_str(self, v: &str) -> Result<Call, Error> {
        Ok(Call {
            method: "serialize_str",
            arg: v.to_string(),
        })
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Call, Error> {
        Ok(Call {
            method: "serialize_bytes",
            arg: format!("{} bytes", v.len()),
        })
    }

    fn serialize_none(self) -> Result<Call, Error> {
        Ok(Call {
            method: "serialize_none",
            arg: String::new(),
        })
    }

    fn serialize_some<T>(self, value: &T) -> Result<Call, Error>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Call, Error> {
        Ok(Call {
            method: "serialize_unit",
            arg: String::new(),
        })
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Call, Error> {
        Ok(Call {
            method: "serialize_unit_struct",
            arg: name.to_string(),
        })
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        variant: &'static str,
    ) -> Result<Call, Error> {
        Ok(Call {
            method: "serialize_unit_variant",
            arg: variant.to_string(),
        })
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<Call, Error>
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
    ) -> Result<Call, Error>
    where
        T: ?Sized + Serialize,
    {
        Err(Error::custom("Trace records scalars only"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        Err(Error::custom("Trace records scalars only"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Err(Error::custom("Trace records scalars only"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Err(Error::custom("Trace records scalars only"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        Err(Error::custom("Trace records scalars only"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        Err(Error::custom("Trace records scalars only"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Err(Error::custom("Trace records scalars only"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        Err(Error::custom("Trace records scalars only"))
    }
}

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();

    out.push_str("one row per primitive_impl! invocation, in source order\n\n");
    out.push_str(&format!(
        "{:<14} {:<20} {}\n",
        "value", "method reached", "argument"
    ));
    out.push_str(&format!("{}\n", "-".repeat(52)));

    // Values are deliberately small. `isize::MAX` would print differently on a
    // 64-bit host and on 32-bit WASM, and the point being made is about the
    // method name, which does not vary.
    macro_rules! row {
        ($value:expr) => {{
            let v = $value;
            let cell = match v.serialize(Trace) {
                Ok(call) => format!("{:<20} {}", call.method, call.arg),
                Err(e) => format!("{:<20} {e}", "(none reached)"),
            };
            out.push_str(&format!("{:<14} {}\n", stringify!($value), cell));
        }};
    }

    row!(true);
    row!(3isize);
    row!(-8i8);
    row!(-16i16);
    row!(-32i32);
    row!(-64i64);
    row!(-128i128);
    row!(3usize);
    row!(8u8);
    row!(16u16);
    row!(32u32);
    row!(64u64);
    row!(128u128);
    row!(1.5f32);
    row!(1.5f64);
    row!('R');

    out.push_str(
        "\nisize and usize have no method of their own: the `as i64` / `as u64`\n\
         tail on their invocation widens them first. The method name above is\n\
         the same on a 64-bit host and on 32-bit WASM.\n",
    );
    out.push_str(
        "\ni128 and u128 are the only two of the sixteen whose Serializer method\n\
         has a default body, and that body errors. Trace never overrode them.\n",
    );

    out
}
