use std::sync::Arc;

use tokio::io::BufReader;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use openssl::rsa::Rsa;
use openssl::symm::{Cipher, Crypter};

// Structures

#[derive(Clone, Debug)]
pub struct NetReceiverHandle {
  sender: mpsc::Sender<NetReceiverMessage>,
}

type ReadHalf = BufReader<OwnedReadHalf>;
const NET_BUFFER_SIZE: usize = 2 * 1024;

pub struct NetReceiverActor {
  pub read_half: ReadHalf,
  encryption_status: EncryptionStatus,
  state: State,
}

#[derive(Debug)]
enum NetReceiverMessage {
  StopActor,
}

enum EncryptionStatus {
  Unencrypted(Arc<(Rsa<openssl::pkey::Private>, Vec<u8>)>),
  Encrypted(Crypter),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  Handshake,
  Encrypt,
  Login,
}

// Implementations

impl NetReceiverHandle {
  pub async fn stop_actor(&mut self) {
    self
      .sender
      .send(NetReceiverMessage::StopActor)
      .await
      .expect("NetReceiverActor was dropped, oopsie!")
  }
}

impl NetReceiverActor {
  pub fn new(
    read_half: OwnedReadHalf,
    encryption: Arc<(Rsa<openssl::pkey::Private>, Vec<u8>)>,
  ) -> Self {
    Self {
      read_half: BufReader::with_capacity(NET_BUFFER_SIZE, read_half),
      encryption_status: EncryptionStatus::Unencrypted(encryption),
      state: State::Handshake,
    }
  }
  pub fn spawn(self) -> (NetReceiverHandle, JoinHandle<NetReceiverActor>) {
    let (send, recv) = mpsc::channel(1024);

    (
      NetReceiverHandle { sender: send },
      tokio::spawn(async move { self.actor(recv).await }),
    )
  }
  async fn actor(mut self, mut recv: mpsc::Receiver<NetReceiverMessage>) -> Self {
    loop {
      tokio::select! {
          // Used only for termination
          msg = recv.recv() => {
              if let Some(msg) = msg {
                  if !self.process_msg(msg).await {
                      break;
                  }
              } else {
                  break;
              }
          }
          result = self.read_packet() => {
              match result {
                  Ok((packet_id, packet_body)) => {
                      // DEBUG: dump packet
                      println!("Received packet ({:#X})", packet_id);
                      for byte in &packet_body {
                          print!("{:02x}", byte);
                      }
                      println!();

                      if let Err(_) = self.process_packet(packet_id, &packet_body).await {
                          break;
                      }
                  }
                  Err(_) => break
              }
          }
      }
    }
    self
  }

  async fn process_msg(&mut self, msg: NetReceiverMessage) -> bool {
    match msg {
      NetReceiverMessage::StopActor => return false,
      //_ => true,
    }
  }

  async fn read_packet(&mut self) -> Result<(u16, Vec<u8>), ()> {
    use byteorder::{LittleEndian, ReadBytesExt};
    use std::convert::TryInto;
    use tokio::io::AsyncReadExt;
    // The packet header is always 6 bytes long
    let mut header = [0; 6];

    // Read header
    match self.read_half.read_exact(&mut header).await {
      Ok(bytes) => {
        debug_assert_eq!(bytes, header.len());

        // Parse header (packet id [2 bytes] + body length [4 bytes])
        let mut header_slice = &header[..];
        let packet_id = ReadBytesExt::read_u16::<LittleEndian>(&mut header_slice).unwrap();
        let body_len = ReadBytesExt::read_u32::<LittleEndian>(&mut header_slice)
          .unwrap()
          .try_into()
          .unwrap();
        let mut body = vec![0; body_len];

        // Read body
        match self.read_half.read_exact(&mut body).await {
          Ok(bytes) => {
            if bytes != body_len {
              #[cfg(debug_assertions)]
              {
                eprintln!(
                  "Unexpected body length (expected {}, found {})",
                  body_len, bytes
                )
              }
              return Err(());
            }

            Ok((packet_id, body))
          }
          Err(_) => Err(()),
        }
      }
      Err(_) => return Err(()),
    }
  }

  async fn process_packet(&mut self, id: u16, mut data: &[u8]) -> Result<(), ()> {
    use super::packet::{self, serial::SerialRead, IngoingPacket};

    macro_rules! switch {
      {$id:expr , $data:expr ; $($pv:ident = $P:ident => $code:block)*} => {
        match $id {
          $(
            $crate::game::net::packet::$P::ID => {
              if self.state != packet::$P::STATE {
                return Err(());
              }
              let $pv = packet::$P::read(&mut $data)?;
              $code
              Ok(())
            }
          )*
          _ => Err(())
        }
      };
    }

    switch! {
      id, data;
      packet = HandshakePacket => {
        println!("Action: {:?}", packet.action);
      }
      packet = EncryptionResponsePacket => {
        println!("Got EncryptionResponsePacket");
      }
    }
  }
}
