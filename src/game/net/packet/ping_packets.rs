use super::{
  serial::{SerialRead, SerialWrite},
  IngoingPacket, OutgoingPacket, State,
};

#[derive(Clone)]
pub struct PingStatusPacket {
  pub players: u32,
  pub games: u32,
  pub status: String,
}

#[derive(Clone)]
pub struct PingPongPacket {
  pub random: u64,
}

impl SerialWrite for PingStatusPacket {
  fn write_consume(self, buf: &mut Vec<u8>) {
    SerialWrite::write_consume(self.players, buf);
    SerialWrite::write_consume(self.games, buf);

    debug_assert!(self.status.len() <= 0xff_ff_ff_ff);
    SerialWrite::write_consume(self.status.len() as u32, buf);
    buf.append(&mut self.status.into_bytes());
  }
}

impl OutgoingPacket for PingStatusPacket {
  const ID: u16 = 0;
  const STATE: State = State::Ping;
}

impl SerialRead for PingPongPacket {
  fn read(data: &mut &[u8]) -> Result<Self, ()> {
    Ok(Self {
      random: SerialRead::read(data)?,
    })
  }
}

impl SerialWrite for PingPongPacket {
  fn write_consume(self, buf: &mut Vec<u8>) {
    SerialWrite::write_consume(self.random, buf);
  }
}

impl IngoingPacket for PingPongPacket {
  const ID: u16 = 1;
  const STATE: State = State::Ping;
}

impl OutgoingPacket for PingPongPacket {
  const ID: u16 = 1;
  const STATE: State = State::Ping;
}
