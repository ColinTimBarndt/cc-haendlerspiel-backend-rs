use super::{serial::SerialRead, IngoingPacket, State};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

#[derive(Debug)]
pub struct HandshakePacket {
  pub action: HandshakeAction,
}

#[derive(Debug, FromPrimitive)]
pub enum HandshakeAction {
  Ping = 1,
  Connect = 2,
}

impl SerialRead for HandshakePacket {
  fn read(data: &mut &[u8]) -> Result<Self, ()> {
    Ok(Self {
      action: SerialRead::read(data)?,
    })
  }
}

impl IngoingPacket for HandshakePacket {
  const ID: u16 = 0;
  const STATE: State = State::Handshake;
}

impl SerialRead for HandshakeAction {
  fn read(data: &mut &[u8]) -> Result<Self, ()> {
    if let Some(v) = FromPrimitive::from_u8(SerialRead::read(data)?) {
      Ok(v)
    } else {
      Err(())
    }
  }
}
