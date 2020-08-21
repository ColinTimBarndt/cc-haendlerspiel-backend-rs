pub mod handshake;
pub mod login;
pub mod ping;

pub mod serial;

pub use handshake::*;
pub use login::*;
pub use ping::*;

use super::State;

pub trait IngoingPacket: serial::SerialRead {
    const ID: u16;
    const STATE: State;
}

pub trait OutgoingPacket: serial::SerialWrite {
    const ID: u16;
    const STATE: State;
}
