use tokio::net::tcp::OwnedWriteHalf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::packet::serial::SerialWrite;
use super::packet::OutgoingPacket;

use openssl::symm::{Cipher, Crypter, Mode};

// Structures

#[derive(Clone, Debug)]
pub struct NetSenderHandle {
  sender: mpsc::Sender<NetSenderMessage>,
}

#[derive(Debug)]
pub struct NetSenderActor {
  pub write_half: OwnedWriteHalf,
  encryption_status: EncryptionStatus,
}

#[derive(Debug)]
enum NetSenderMessage {
  StopActor,
  SendPacket(Vec<u8>),
  Encrypt(Vec<u8>),
}

enum EncryptionStatus {
  Unencrypted,
  Encrypted(Crypter),
}

impl std::fmt::Debug for EncryptionStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let encrypted = if let Self::Unencrypted = self {
      false
    } else {
      true
    };
    f.debug_struct("EncryptionStatus")
      .field("encrypted", &encrypted)
      .finish()
  }
}

// Implementations

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
  pub async fn encrypt(&mut self, secret: Vec<u8>) {
    self
      .sender
      .send(NetSenderMessage::Encrypt(secret))
      .await
      .expect(ACTOR_DROPPED_ERROR)
  }
}

impl NetSenderActor {
  pub fn new(write_half: OwnedWriteHalf) -> Self {
    Self {
      write_half,
      encryption_status: EncryptionStatus::Unencrypted,
    }
  }
  pub fn spawn(self) -> (NetSenderHandle, JoinHandle<NetSenderActor>) {
    let (send, recv) = mpsc::channel(1024);

    (
      NetSenderHandle { sender: send },
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
        Some(NetSenderMessage::SendPacket(data)) => match &mut self.encryption_status {
          EncryptionStatus::Unencrypted => {
            if let Result::Err(_err) = self.write_half.write(&data).await {
              return self;
            }
            println!("Sending packet");
            for byte in &data {
              print!("{:02X}", byte);
            }
            println!();
          }
          EncryptionStatus::Encrypted(crypter) => {
            let mut buf = vec![0u8; data.len()];
            let enc_len = crypter
              .update(&data, &mut buf[..])
              .expect("Encryption error");
            debug_assert_eq!(enc_len, buf.len());
            if let Result::Err(_err) = self.write_half.write(&buf).await {
              return self;
            }
          }
        },
        Some(NetSenderMessage::Encrypt(secret)) => {
          let cipher = Cipher::aes_128_cfb8();
          let crypter = Crypter::new(cipher, Mode::Encrypt, &secret, Some(&secret)).unwrap();
          self.encryption_status = EncryptionStatus::Encrypted(crypter);
        }
      }
    }
  }
}
