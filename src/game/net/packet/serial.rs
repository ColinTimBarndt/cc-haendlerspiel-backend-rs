//! Module containing helper implementations for reading and writing data in serial

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

pub trait SerialWrite: Sized + Clone {
  fn write(&self, buf: &mut Vec<u8>) {
    self.clone().write_consume(buf);
  }
  fn write_consume(self, buf: &mut Vec<u8>) {
    self.write(buf);
  }
}

pub trait SerialRead: Sized {
  fn read(data: &mut &[u8]) -> Result<Self, ()>;
}

// Writing

impl SerialWrite for u8 {
  fn write_consume(self, buf: &mut Vec<u8>) {
    buf.push(self);
  }
}

impl SerialWrite for i8 {
  fn write_consume(self, buf: &mut Vec<u8>) {
    buf.push(self as u8);
  }
}

impl SerialWrite for f32 {
  fn write_consume(self, buf: &mut Vec<u8>) {
    buf.write_f32::<LittleEndian>(self).unwrap();
  }
}

impl SerialWrite for f64 {
  fn write_consume(self, buf: &mut Vec<u8>) {
    buf.write_f64::<LittleEndian>(self).unwrap();
  }
}

impl SerialWrite for &'_ [u8] {
  fn write_consume(self, buf: &mut Vec<u8>) {
    buf.extend(self.into_iter());
  }
}

macro_rules! impl_write {
  ($T:ty) => {
    impl SerialWrite for $T {
      fn write_consume(self, buf: &mut Vec<u8>) {
        buf.extend(self.to_le_bytes().iter());
      }
    }
  };
  ($($T:ty),*) => {
    $(
      impl_write!($T);
    )*
  }
}

impl_write!(u16, u32, u64, u128);
impl_write!(i16, i32, i64, i128);

// Reading

impl SerialRead for u8 {
  fn read(data: &mut &[u8]) -> Result<Self, ()> {
    if data.len() != 0 {
      let byte = data[0];
      *data = &data[1..];
      Ok(byte)
    } else {
      Err(())
    }
  }
}

impl SerialRead for i8 {
  fn read(data: &mut &[u8]) -> Result<Self, ()> {
    let byte: u8 = SerialRead::read(data)?;
    Ok(byte as i8)
  }
}

macro_rules! impl_write {
  ($T:ty : $w:literal : $r:ident) => {
    impl SerialRead for $T {
      fn read(data: &mut &[u8]) -> Result<Self, ()> {
        if data.len() >= $w {
          if let Ok(res) = ReadBytesExt::$r::<LittleEndian>(data) {
            return Ok(res);
          }
        }
        Err(())
      }
    }
  };
  ($($T:ty : $w:literal : $r:ident),*) => {
    $(
      impl_write!($T : $w : $r);
    )*
  }
}

impl_write!(u16:2:read_u16, u32:4:read_u32, u64:4:read_u64, u128:4:read_u128);
impl_write!(i16:2:read_i16, i32:4:read_i32, i64:4:read_i64, i128:4:read_i128);
impl_write!(f32:4:read_f32, f64:8:read_f64);

// Strings

macro_rules! impl_string {
  ($T:ident , $LT:ty) => {
    #[derive(Clone)]
    pub struct $T {
      inner: String,
    }

    impl From<String> for $T {
      fn from(s: String) -> Self {
        Self { inner: s }
      }
    }

    impl From<$T> for String {
      fn from(s: $T) -> Self {
        s.inner
      }
    }

    impl std::ops::Deref for $T {
      type Target = String;
      fn deref(&self) -> &String {
        &self.inner
      }
    }

    impl SerialRead for $T {
      fn read(data: &mut &[u8]) -> Result<Self, ()> {
        let len: usize = {
          use std::convert::TryInto;
          let len: $LT = SerialRead::read(data)?;
          len.try_into().map_err(|_| ())?
        };
        let s_data = &data[..len];
        *data = &data[len..];
        Ok(String::from_utf8(Vec::from(s_data)).map_err(|_| ())?.into())
      }
    }

    impl SerialWrite for $T {
      fn write_consume(self, buf: &mut Vec<u8>) {
        use std::convert::TryInto;
        SerialWrite::write_consume(
          {
            let l: $LT = self
              .inner
              .len()
              .try_into()
              .expect("String length exceeds limits of u32");
            l
          },
          buf,
        );
        buf.extend(self.inner.into_bytes());
      }
    }
  };
}

impl_string!(PacketString, u32);
impl_string!(PacketNameString, u8);

// Lists

#[derive(Clone)]
pub struct PacketList<T> {
  inner: Vec<T>,
}

impl<T> From<Vec<T>> for PacketList<T> {
  fn from(inner: Vec<T>) -> Self {
    Self { inner }
  }
}

impl<T> From<PacketList<T>> for Vec<T> {
  fn from(s: PacketList<T>) -> Self {
    s.inner
  }
}

impl<T> std::ops::Deref for PacketList<T> {
  type Target = Vec<T>;
  fn deref(&self) -> &Vec<T> {
    &self.inner
  }
}

impl<T: SerialRead> SerialRead for PacketList<T> {
  fn read(data: &mut &[u8]) -> Result<Self, ()> {
    let len: usize = {
      use std::convert::TryInto;
      let len: u32 = SerialRead::read(data)?;
      len.try_into().map_err(|_| ())?
    };
    let mut vec: Vec<T> = Vec::with_capacity(len);
    for _ in 0..len {
      vec.push(SerialRead::read(data)?);
    }
    Ok(vec.into())
  }
}

impl<T: SerialWrite> SerialWrite for PacketList<T> {
  fn write_consume(self, buf: &mut Vec<u8>) {
    use std::convert::TryInto;
    SerialWrite::write_consume(
      {
        let l: u32 = self
          .inner
          .len()
          .try_into()
          .expect("Vec length exceeds limits of u32");
        l
      },
      buf,
    );
    for e in self.inner {
      SerialWrite::write_consume(e, buf);
    }
  }
}
