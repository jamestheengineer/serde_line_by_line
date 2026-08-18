//! Example: `Serializer::Ok` is chosen by the *format*, not by Serde.
//!
//! Two serializers, same trait, same input values. `AsText` produces a
//! `String`; `ByteCount` produces a `usize`. Neither the values nor the
//! `Serialize` impls know the difference — that is the whole point of
//! `type Ok`.
//!
//! Both are scalar-only: the compound-type associated types are filled with
//! `Impossible`, which is serde_core's way of saying "this format cannot build
//! sequences or maps" while still satisfying the trait.

use serde_core::ser::{Error as _, Impossible, Serialize, Serializer};
use std::fmt::{self, Display};

/// A serializer that renders scalars to text.
#[derive(Clone, Copy)]
pub struct AsText;

/// A serializer that reports how many bytes the text form would occupy.
#[derive(Clone, Copy)]
pub struct ByteCount;

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

/// Generates a scalar-only `Serializer`. The only difference between the two
/// implementations is `$ok` and how the rendered text is converted into it.
///
/// serde_core does the same thing for the same reason: see the `primitive!`
/// macro in `src/ser/impls.rs`.
macro_rules! scalar_serializer {
    ($name:ty, $ok:ty, |$text:ident| $finish:expr) => {
        impl Serializer for $name {
            type Ok = $ok;
            type Error = Error;

            type SerializeSeq = Impossible<$ok, Error>;
            type SerializeTuple = Impossible<$ok, Error>;
            type SerializeTupleStruct = Impossible<$ok, Error>;
            type SerializeTupleVariant = Impossible<$ok, Error>;
            type SerializeMap = Impossible<$ok, Error>;
            type SerializeStruct = Impossible<$ok, Error>;
            type SerializeStructVariant = Impossible<$ok, Error>;

            fn serialize_bool(self, v: bool) -> Result<$ok, Error> {
                let $text = v.to_string();
                Ok($finish)
            }
            fn serialize_i8(self, v: i8) -> Result<$ok, Error> {
                self.serialize_i64(v as i64)
            }
            fn serialize_i16(self, v: i16) -> Result<$ok, Error> {
                self.serialize_i64(v as i64)
            }
            fn serialize_i32(self, v: i32) -> Result<$ok, Error> {
                self.serialize_i64(v as i64)
            }
            fn serialize_i64(self, v: i64) -> Result<$ok, Error> {
                let $text = v.to_string();
                Ok($finish)
            }
            fn serialize_u8(self, v: u8) -> Result<$ok, Error> {
                self.serialize_u64(v as u64)
            }
            fn serialize_u16(self, v: u16) -> Result<$ok, Error> {
                self.serialize_u64(v as u64)
            }
            fn serialize_u32(self, v: u32) -> Result<$ok, Error> {
                self.serialize_u64(v as u64)
            }
            fn serialize_u64(self, v: u64) -> Result<$ok, Error> {
                let $text = v.to_string();
                Ok($finish)
            }
            fn serialize_f32(self, v: f32) -> Result<$ok, Error> {
                self.serialize_f64(v as f64)
            }
            fn serialize_f64(self, v: f64) -> Result<$ok, Error> {
                let $text = v.to_string();
                Ok($finish)
            }
            fn serialize_char(self, v: char) -> Result<$ok, Error> {
                let $text = v.to_string();
                Ok($finish)
            }
            fn serialize_str(self, v: &str) -> Result<$ok, Error> {
                let $text = v.to_string();
                Ok($finish)
            }
            fn serialize_bytes(self, v: &[u8]) -> Result<$ok, Error> {
                let $text = format!("<{} bytes>", v.len());
                Ok($finish)
            }
            fn serialize_none(self) -> Result<$ok, Error> {
                self.serialize_unit()
            }
            fn serialize_some<T>(self, value: &T) -> Result<$ok, Error>
            where
                T: ?Sized + Serialize,
            {
                value.serialize(self)
            }
            fn serialize_unit(self) -> Result<$ok, Error> {
                let $text = String::from("null");
                Ok($finish)
            }
            fn serialize_unit_struct(self, _name: &'static str) -> Result<$ok, Error> {
                self.serialize_unit()
            }
            fn serialize_unit_variant(
                self,
                _name: &'static str,
                _index: u32,
                variant: &'static str,
            ) -> Result<$ok, Error> {
                self.serialize_str(variant)
            }
            fn serialize_newtype_struct<T>(
                self,
                _name: &'static str,
                value: &T,
            ) -> Result<$ok, Error>
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
            ) -> Result<$ok, Error>
            where
                T: ?Sized + Serialize,
            {
                Err(Error::custom("this format supports scalars only"))
            }
            fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
                Err(Error::custom("this format supports scalars only"))
            }
            fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
                Err(Error::custom("this format supports scalars only"))
            }
            fn serialize_tuple_struct(
                self,
                _name: &'static str,
                _len: usize,
            ) -> Result<Self::SerializeTupleStruct, Error> {
                Err(Error::custom("this format supports scalars only"))
            }
            fn serialize_tuple_variant(
                self,
                _name: &'static str,
                _index: u32,
                _variant: &'static str,
                _len: usize,
            ) -> Result<Self::SerializeTupleVariant, Error> {
                Err(Error::custom("this format supports scalars only"))
            }
            fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
                Err(Error::custom("this format supports scalars only"))
            }
            fn serialize_struct(
                self,
                _name: &'static str,
                _len: usize,
            ) -> Result<Self::SerializeStruct, Error> {
                Err(Error::custom("this format supports scalars only"))
            }
            fn serialize_struct_variant(
                self,
                _name: &'static str,
                _index: u32,
                _variant: &'static str,
                _len: usize,
            ) -> Result<Self::SerializeStructVariant, Error> {
                Err(Error::custom("this format supports scalars only"))
            }
        }
    };
}

scalar_serializer!(AsText, String, |text| text);
scalar_serializer!(ByteCount, usize, |text| text.len());

/// Every example exposes `run() -> String` rather than printing, so the same
/// code runs unchanged under `cargo test` and as WASM in the browser.
pub fn run() -> String {
    let mut out = String::new();

    out.push_str("AsText     -> type Ok = String\n");
    out.push_str("ByteCount  -> type Ok = usize\n\n");

    macro_rules! show {
        ($value:expr) => {{
            let v = $value;
            let text = v.serialize(AsText).unwrap();
            let count = v.serialize(ByteCount).unwrap();
            // Note: `{:>10?}` would NOT pad here — Debug impls ignore width
            // unless they call `Formatter::pad`. Render first, then align.
            let quoted = format!("{text:?}");
            out.push_str(&format!(
                "{:<20} {:>10} {:>6}\n",
                stringify!($value),
                quoted,
                count
            ));
        }};
    }

    out.push_str(&format!(
        "{:<20} {:>10} {:>6}\n",
        "value", "AsText", "bytes"
    ));
    show!(true);
    show!(-42i32);
    show!(7u8);
    show!(1.5f64);
    show!('R');
    show!("serde");
    show!(Option::<u8>::None);
    show!(Some(9u8));

    out.push_str("\nThe values never learn which format they were given to.\n");

    let err = vec![1, 2, 3].serialize(AsText).unwrap_err();
    out.push_str(&format!("compound types are refused: {err}\n"));

    out
}
