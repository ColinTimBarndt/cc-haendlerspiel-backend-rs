use super::{
  serial::{SerialRead, SerialWrite},
  IngoingPacket, OutgoingPacket, State,
};
use std::sync::Arc;

use openssl::rsa::{Padding, Rsa};

#[derive(Clone)]
pub struct RequestEncryptionPacket {
  pub public_key: Arc<Vec<u8>>,
  pub verify: Arc<Vec<u8>>,
}

pub struct EncryptionResponsePacket {
  pub verify: Vec<u8>,
  pub secret: Vec<u8>,
}

impl SerialWrite for RequestEncryptionPacket {
  fn write_consume(self, buf: &mut Vec<u8>) {
    SerialWrite::write_consume(self.public_key.len() as u32, buf);
    SerialWrite::write_consume(&self.public_key[..], buf);
    SerialWrite::write_consume(self.verify.len() as u32, buf);
    SerialWrite::write_consume(&self.verify[..], buf);
  }
}

impl OutgoingPacket for RequestEncryptionPacket {
  const ID: u16 = 1;
}

impl EncryptionResponsePacket {
  /// Verifies that the bytes were encrypted
  /// correctly and returns the decrypted
  /// shared secret.
  pub fn verify(
    self,
    rsa: Arc<Rsa<openssl::pkey::Private>>,
    correct_verify: &[u8],
  ) -> Result<Vec<u8>, ()> {
    let mut buf = vec![0; rsa.size() as usize];
    let len = rsa
      .private_decrypt(&self.verify, &mut buf, Padding::PKCS1)
      .unwrap();
    if &buf[0..len] != correct_verify {
      Err(())
    } else {
      let len = rsa
        .private_decrypt(&self.secret, &mut buf, Padding::PKCS1)
        .unwrap();
      Ok(Vec::from(&buf[0..len]))
    }
  }
}

impl SerialRead for EncryptionResponsePacket {
  fn read(data: &mut &[u8]) -> Result<Self, ()> {
    let len = {
      let len: u32 = SerialRead::read(data)?;
      len as usize
    };
    let verify = &data[0..len];
    *data = &data[len..];
    let secret = &data[0..];
    Ok(Self {
      verify: Vec::from(verify),
      secret: Vec::from(secret),
    })
  }
}

impl IngoingPacket for EncryptionResponsePacket {
  const ID: u16 = 1;
  const STATE: State = State::Encrypt;
}
