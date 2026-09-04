use serde::{Serializer, ser};

#[derive(Debug)]
pub struct AmeonSer {}

#[derive(Debug)]
pub enum AmeSerError {
    Custom(String),
}

impl std::fmt::Display for AmeSerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom(t) => t.fmt(f),
        }
    }
}

impl std::error::Error for AmeSerError {}

impl ser::Error for AmeSerError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::Custom(msg.to_string())
    }
}

#[derive(Debug)]
pub struct AmeSerSeq {}

impl ser::SerializeSeq for AmeSerSeq {
    type Ok = ();
    type Error = AmeSerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct AmeSerTuple {}

impl ser::SerializeTuple for AmeSerTuple {
    type Ok = ();
    type Error = AmeSerError;

    fn serialize_element<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct AmeSerTupleStruct {}

impl ser::SerializeTupleStruct for AmeSerTupleStruct {
    type Ok = ();
    type Error = AmeSerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct AmeSerTupleVariant {}

impl ser::SerializeTupleVariant for AmeSerTupleVariant {
    type Ok = ();
    type Error = AmeSerError;

    fn serialize_field<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct AmeSerMap {}

impl ser::SerializeMap for AmeSerMap {
    type Ok = ();
    type Error = AmeSerError;

    fn serialize_key<T>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        todo!()
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct AmeSerStruct {}

impl ser::SerializeStruct for AmeSerStruct {
    type Ok = ();
    type Error = AmeSerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

#[derive(Debug)]
pub struct AmeSerStructVariant {}

impl ser::SerializeStructVariant for AmeSerStructVariant {
    type Ok = ();
    type Error = AmeSerError;

    fn serialize_field<T>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error>
    where
        T: ?Sized + ser::Serialize,
    {
        todo!()
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }
}

impl Serializer for AmeonSer {
    type Ok = ();
    type Error = AmeSerError;
    type SerializeSeq = AmeSerSeq;
    type SerializeTuple = AmeSerTuple;
    type SerializeTupleStruct = AmeSerTupleStruct;
    type SerializeTupleVariant = AmeSerTupleVariant;
    type SerializeMap = AmeSerMap;
    type SerializeStruct = AmeSerStruct;
    type SerializeStructVariant = AmeSerStructVariant;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_some<T>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        todo!()
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_unit_struct(self, name: &'static str) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_unit_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn serialize_newtype_struct<T>(
        self,
        name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        todo!()
    }

    fn serialize_newtype_variant<T>(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: ?Sized + serde::Serialize,
    {
        todo!()
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        todo!()
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        todo!()
    }

    fn serialize_tuple_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        todo!()
    }

    fn serialize_tuple_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        todo!()
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        todo!()
    }

    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        todo!()
    }

    fn serialize_struct_variant(
        self,
        name: &'static str,
        variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        todo!()
    }
}
