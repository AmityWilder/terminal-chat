use crate::error::{Error, Result};
use serde::{
    Serialize,
    ser::{
        SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
        SerializeTupleStruct, SerializeTupleVariant,
    },
};

#[derive(Debug)]
pub struct Serializer {
    output: Vec<u8>,
}

impl Serializer {
    fn push(&mut self, byte: u8) {
        self.output.push(byte);
    }

    fn push_bytes(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
    }

    fn push_usize(&mut self, n: usize) -> Result<()> {
        self.push_bytes(&u64::try_from(n)?.to_be_bytes());
        Ok(())
    }

    fn push_vidx(&mut self, variant_index: u32) {
        self.push(
            u8::try_from(variant_index).expect("didn't anticipate an enum having 256+ variants"),
        )
    }
}

pub fn to_bytes<T>(value: &T) -> Result<Vec<u8>>
where
    T: ?Sized + Serialize,
{
    let mut serializer = Serializer { output: Vec::new() };
    value.serialize(&mut serializer)?;
    Ok(serializer.output)
}

impl serde::Serializer for &mut Serializer {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<()> {
        self.push(v as u8);
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<()> {
        self.push(v.cast_unsigned());
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<()> {
        self.push_bytes(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<()> {
        self.push_bytes(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<()> {
        self.push_bytes(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_i128(self, v: i128) -> Result<()> {
        self.push_bytes(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<()> {
        self.push(v);
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<()> {
        self.push_bytes(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<()> {
        self.push_bytes(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<()> {
        self.push_bytes(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_u128(self, v: u128) -> Result<()> {
        self.push_bytes(&v.to_be_bytes());
        Ok(())
    }

    fn serialize_f32(self, _v: f32) -> Result<()> {
        unimplemented!()
    }

    fn serialize_f64(self, _v: f64) -> Result<()> {
        unimplemented!()
    }

    fn serialize_char(self, v: char) -> Result<()> {
        self.push_bytes(v.encode_utf8(&mut [0; char::MAX_LEN_UTF8]).as_bytes());
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<()> {
        self.serialize_bytes(v.as_bytes())
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<()> {
        self.push_usize(v.len())?;
        self.push_bytes(v);
        Ok(())
    }

    fn serialize_none(self) -> Result<()> {
        self.push(0);
        Ok(())
    }

    fn serialize_some<T>(self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.push(1);
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<()> {
        unimplemented!()
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<()> {
        unimplemented!()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32, // this feels wasteful, most are only 1 byte :c
        _variant: &'static str,
    ) -> Result<()> {
        self.push_vidx(variant_index);
        Ok(())
    }

    fn serialize_newtype_struct<T>(self, _name: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        self.push_vidx(variant_index);
        value.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq> {
        self.push_usize(len.unwrap_or_else(|| unimplemented!()))?;
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple> {
        Ok(self)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct> {
        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant> {
        self.push_vidx(variant_index);
        Ok(self)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap> {
        self.serialize_seq(len)
    }

    fn serialize_struct(self, _name: &'static str, _len: usize) -> Result<Self::SerializeStruct> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant> {
        self.push_vidx(variant_index);
        Ok(self)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

impl SerializeSeq for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl SerializeTuple for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_element<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl SerializeTupleStruct for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl SerializeTupleVariant for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl SerializeMap for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_key<T>(&mut self, key: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        key.serialize(&mut **self)
    }

    fn serialize_value<T>(&mut self, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl SerializeStruct for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}

impl SerializeStructVariant for &mut Serializer {
    type Ok = ();
    type Error = Error;

    fn serialize_field<T>(&mut self, _key: &'static str, value: &T) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        value.serialize(&mut **self)
    }

    fn end(self) -> Result<()> {
        Ok(())
    }
}
