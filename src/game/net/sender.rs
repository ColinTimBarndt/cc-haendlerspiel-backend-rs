use std::net::SocketAddr;
use std::{cmp, hash::Hasher};

use tokio::io::BufWriter;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::packet::serial::SerialWrite;
use super::packet::OutgoingPacket;

// Structures

#[derive(Clone, Debug)]
pub struct NetSenderHandle {
  sender: mpsc::Sender<NetSenderMessage>,
  address: SocketAddr,
}

type WriteHalf = tokio::io::WriteHalf<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>;
type Writer = BufWriter<WriteHalf>;
const NET_BUFFER_SIZE: usize = 2 * 1024;

pub struct NetSenderActor {
  pub write_half: Writer,
  pub address: SocketAddr,
}

#[derive(Debug)]
enum NetSenderMessage {
  StopActor,
  SendPacket(Vec<u8>),
}

// Implementations

impl From<NetSenderActor> for WriteHalf {
  fn from(actor: NetSenderActor) -> Self {
    actor.write_half.into_inner()
  }
}

impl cmp::PartialEq for NetSenderHandle {
  fn eq(&self, other: &Self) -> bool {
    self.address == other.address
  }
}

impl std::hash::Hash for NetSenderHandle {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.address.hash(state);
  }
}

const ACTOR_DROPPED_ERROR: &'static str = "NetSenderActor was dropped, oopsie!";

impl NetSenderHandle {
  pub async fn stop_actor(&mut self) {
    self
      .sender
      .send(NetSenderMessage::StopActor)
      .await
      .expect(ACTOR_DROPPED_ERROR)
  }
  pub async fn send_packet<P: OutgoingPacket>(&mut self, packet: P) {
    let mut buf = Vec::with_capacity(40);
    SerialWrite::write_consume(P::ID, &mut buf);

    let mut pbuf = Vec::with_capacity(32);
    SerialWrite::write_consume(packet, &mut pbuf);

    SerialWrite::write_consume(
      std::convert::TryInto::<u32>::try_into(pbuf.len()).expect("Packet is too large!"),
      &mut buf,
    );

    buf.append(&mut pbuf);

    self
      .sender
      .send(NetSenderMessage::SendPacket(buf))
      .await
      .expect(ACTOR_DROPPED_ERROR)
  }
}

impl NetSenderActor {
  pub fn new(write_half: WriteHalf, address: SocketAddr) -> Self {
    Self {
      write_half: BufWriter::with_capacity(NET_BUFFER_SIZE, write_half),
      address,
    }
  }
  pub fn spawn(self) -> (NetSenderHandle, JoinHandle<NetSenderActor>) {
    let (send, recv) = mpsc::channel(1024);

    (
      NetSenderHandle {
        sender: send,
        address: self.address.clone(),
      },
      tokio::spawn(async move { self.actor(recv).await }),
    )
  }
  async fn actor(mut self, mut recv: mpsc::Receiver<NetSenderMessage>) -> Self {
    use tokio::io::AsyncWriteExt;
    use tokio::io::Result;
    loop {
      match recv.recv().await {
        None => return self,
        Some(NetSenderMessage::StopActor) => return self,
        Some(NetSenderMessage::SendPacket(data)) => {
          if let Result::Err(_err) = self.write_half.write(&data).await {
            return self;
          }
          println!("Sending packet");
          for byte in &data {
            print!("{:02X}", byte);
          }
          println!();
        }
      }
    }
  }
}
