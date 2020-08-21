use super::{
  serial::{PacketList, PacketNameString, SerialRead, SerialWrite},
  IngoingPacket, OutgoingPacket, State,
};

// Structures

#[derive(Clone)]
pub struct ListGamesPacket {
  pub entries: Vec<ListGamesEntry>,
}

#[derive(Clone)]
pub enum ListGamesEntry {
  Add { id: u64, name: String, players: u32 },
  Remove { id: u64 },
}

pub struct SyncGamesPacket {}

pub struct LoginPacket {
  pub username: String,
  pub password: String,
}

#[derive(Clone)]
pub struct LoginResponsePacket {
  pub permission_level: crate::game::permission_level::PermissionLevel,
}

// Implementations

impl SerialWrite for ListGamesPacket {
  fn write_consume(self, buf: &mut Vec<u8>) {
    SerialWrite::write_consume(PacketList::from(self.entries), buf);
  }
}

impl SerialWrite for ListGamesEntry {
  fn write_consume(self, buf: &mut Vec<u8>) {
    match self {
      Self::Add { id, name, players } => {
        SerialWrite::write_consume(id, buf);
        SerialWrite::write_consume(0u8, buf);
        SerialWrite::write_consume(PacketNameString::from(name), buf);
        SerialWrite::write_consume(players, buf);
      }
      Self::Remove { id } => {
        SerialWrite::write_consume(id, buf);
        SerialWrite::write_consume(1u8, buf);
      }
    }
  }
}

impl OutgoingPacket for ListGamesPacket {
  const ID: u16 = 0;
  const STATE: State = State::Login;
}

impl SerialRead for SyncGamesPacket {
  fn read(_buf: &mut &[u8]) -> Result<Self, ()> {
    Ok(Self {})
  }
}

impl IngoingPacket for SyncGamesPacket {
  const ID: u16 = 0;
  const STATE: State = State::Login;
}

impl SerialRead for LoginPacket {
  fn read(buf: &mut &[u8]) -> Result<Self, ()> {
    let user: PacketNameString = SerialRead::read(buf)?;
    let pass: PacketNameString = SerialRead::read(buf)?;
    Ok(Self {
      username: user.into(),
      password: pass.into(),
    })
  }
}

impl IngoingPacket for LoginPacket {
  const ID: u16 = 1;
  const STATE: State = State::Login;
}

impl SerialWrite for LoginResponsePacket {
  fn write_consume(self, buf: &mut Vec<u8>) {
    SerialWrite::write_consume(self.permission_level as u8, buf);
  }
}

impl OutgoingPacket for LoginResponsePacket {
  const ID: u16 = 1;
  const STATE: State = State::Login;
}
