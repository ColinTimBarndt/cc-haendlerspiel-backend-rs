use super::{
  serial::{SerialRead, SerialWrite},
  IngoingPacket, OutgoingPacket, State,
};
use std::sync::Arc;

use openssl::rsa::{Padding, Rsa};

type RsaArc = Arc<(Rsa<openssl::pkey::Private>, Vec<u8>)>;

#[derive(Clone)]
pub struct RequestEncryptionPacket {
  // Using this weird Arc for performance, only the Vec<u8> is used
  // (so that no new Arcs need to be created)
  pub public_key: RsaArc,
  pub verify: Arc<Vec<u8>>,
}

pub struct EncryptionResponsePacket {
  pub verify: Vec<u8>,
  pub secret: Vec<u8>,
}

#[derive(Clone)]
pub struct EncryptionSuccessPacket {}

impl SerialWrite for RequestEncryptionPacket {
  fn write_consume(self, buf: &mut Vec<u8>) {
    SerialWrite::write_consume(self.public_key.1.len() as u32, buf);
    SerialWrite::write_consume(&self.public_key.1[..], buf);
    SerialWrite::write_consume(self.verify.len() as u32, buf);
    SerialWrite::write_consume(&self.verify[..], buf);
  }
}

impl OutgoingPacket for RequestEncryptionPacket {
  const ID: u16 = 0;
  const STATE: State = State::Encrypt;
}

impl EncryptionResponsePacket {
  /// Verifies that the bytes were encrypted
  /// correctly and returns the decrypted
  /// shared secret.
  pub fn verify(
    self,
    // Using this weird Arc for performance, only the Rsa<Private> is used
    // (so that no new Arcs need to be created)
    rsa: &RsaArc,
    correct_verify: &[u8],
  ) -> Result<Vec<u8>, ()> {
    let mut buf = vec![0; rsa.0.size() as usize];
    let len = rsa
      .0
      .private_decrypt(&self.verify, &mut buf, Padding::PKCS1_OAEP)
      .unwrap();
    if &buf[0..len] != correct_verify {
      Err(())
    } else {
      let len = rsa
        .0
        .private_decrypt(&self.secret, &mut buf, Padding::PKCS1_OAEP)
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
    let len = {
      let len: u32 = SerialRead::read(data)?;
      len as usize
    };
    let secret = &data[0..len];
    *data = &data[len..];
    Ok(Self {
      verify: Vec::from(verify),
      secret: Vec::from(secret),
    })
  }
}

impl IngoingPacket for EncryptionResponsePacket {
  const ID: u16 = 0;
  const STATE: State = State::Encrypt;
}

impl SerialWrite for EncryptionSuccessPacket {
  fn write_consume(self, buf: &mut Vec<u8>) {
    SerialWrite::write_consume(0x__DEADBEEF__u32, buf);
  }
}

impl OutgoingPacket for EncryptionSuccessPacket {
  const ID: u16 = 1;
  const STATE: State = State::Encrypt;
}
