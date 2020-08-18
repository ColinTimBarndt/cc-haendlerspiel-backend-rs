mod encryption;
mod handshake_packet;

pub mod serial;
pub use encryption::*;
pub use handshake_packet::*;

use super::State;

pub trait IngoingPacket: serial::SerialRead {
    const ID: u16;
    const STATE: State;
}

pub trait OutgoingPacket: serial::SerialWrite {
    const ID: u16;
}
