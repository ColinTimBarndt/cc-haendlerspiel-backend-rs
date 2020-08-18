use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::BufReader;
use tokio::net::tcp::OwnedReadHalf;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use openssl::rsa::Rsa;
use openssl::symm::{Cipher, Crypter, Mode};

use super::NetSenderHandle;

// Structures

#[derive(Clone, Debug)]
pub struct NetReceiverHandle {
  sender: mpsc::Sender<NetReceiverMessage>,
}

type ReadHalf = BufReader<OwnedReadHalf>;
const NET_BUFFER_SIZE: usize = 2 * 1024;

#[derive(Debug)]
pub struct NetReceiverActor {
  pub read_half: ReadHalf,
  pub sender: NetSenderHandle,
  pub address: SocketAddr,
  encryption_status: EncryptionStatus,
  verify_key: Option<Arc<Vec<u8>>>,
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

impl std::fmt::Debug for EncryptionStatus {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let encrypted = if let Self::Unencrypted(_) = self {
      false
    } else {
      true
    };
    f.debug_struct("EncryptionStatus")
      .field("encrypted", &encrypted)
      .finish()
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
  Handshake,
  Ping,
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
    sender: NetSenderHandle,
    address: SocketAddr,
  ) -> Self {
    Self {
      read_half: BufReader::with_capacity(NET_BUFFER_SIZE, read_half),
      encryption_status: EncryptionStatus::Unencrypted(encryption),
      verify_key: None,
      state: State::Handshake,
      sender,
      address,
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

              match self.process_packet(packet_id, &packet_body).await {
                Err(_) => break,
                Ok(true) => (),
                Ok(false) => break,
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

        // Decrypt
        if let EncryptionStatus::Encrypted(crypter) = &mut self.encryption_status {
          header = decrypt_header(crypter, header);
        }

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

            // Decrypt
            if let EncryptionStatus::Encrypted(crypter) = &mut self.encryption_status {
              body = decrypt_vec(crypter, body);
            }

            Ok((packet_id, body))
          }
          Err(_) => Err(()),
        }
      }
      Err(_) => return Err(()),
    }
  }

  async fn process_packet(&mut self, id: u16, mut data: &[u8]) -> Result<bool, ()> {
    use super::packet::{self, serial::SerialRead, IngoingPacket};

    macro_rules! switch {
      {
        $id:expr , $state:expr , $data:expr ;

        $(
          $pv:ident = $P:ident => $code:block
        )*
      } => {
        match ($id, $state) {
          $(
            ($crate::game::net::packet::$P::ID, $crate::game::net::packet::$P::STATE) => {
              let $pv = packet::$P::read(&mut $data)?;
              if $data.len() != 0 {
                // The packet contained too many bytes
                eprintln!("(⚠) Inflated packet from {}", self.address);
                return Err(());
              }
              $code
            }
          )*
          _ => Err(())
        }
      };
    }

    switch! {
      id, self.state, data;

      // --- State = Handshake ---
      packet = HandshakePacket => {
        println!("Action: {:?}", packet.action);
        use super::packet::HandshakeAction;
        match packet.action {
          HandshakeAction::Ping => {
            use super::packet::PingStatusPacket;
            self.state = State::Ping;
            let response = PingStatusPacket {
              players: 0,
              games: 0,
              status: r#"{"text":"Test server"}"#.into()
            };
            self.sender.send_packet(response).await;
            Ok(true)
          }
          HandshakeAction::Connect => {
            use super::packet::RequestEncryptionPacket;
            self.state = State::Encrypt;
            let mut verify = [0; 64];
            openssl::rand::rand_bytes(&mut verify).unwrap();
            let verify: Arc<Vec<u8>> = Arc::new((&verify[..]).into());
            self.verify_key = Some(verify.clone());

            let pkey = match &self.encryption_status {
              EncryptionStatus::Encrypted(_) => unreachable!(),
              EncryptionStatus::Unencrypted(pk) => pk.clone()
            };

            let response = RequestEncryptionPacket {
              public_key: pkey,
              verify
            };
            self.sender.send_packet(response).await;
            Ok(true)
          }
        }
      }

      // --- State = Ping ---
      packet = PingPongPacket => {
        self.sender.send_packet(packet).await;
        Ok(false)
      }

      // --- State = Encrypt ---
      packet = EncryptionResponsePacket => {
        use super::packet::EncryptionSuccessPacket;
        println!("Got EncryptionResponsePacket");
        if self.verify_key.is_none() {
          return Err(());
        }

        let verify = self.verify_key.as_ref().unwrap();
        let pkey = match &self.encryption_status {
          EncryptionStatus::Encrypted(_) => unreachable!(),
          EncryptionStatus::Unencrypted(pk) => pk
        };

        // If the encryption cannot be verified, the
        // connection is terminated
        let secret = packet.verify(pkey, verify)?;

        // Encrypt the sender
        self.sender.encrypt(secret.clone()).await;

        print!("SECRET: ");
        for byte in &secret {
          print!("{:02X}", byte);
        }
        println!();

        // Encrypt the reader
        {
          let cipher = Cipher::aes_128_cfb8();
          let crypter = Crypter::new(cipher, Mode::Decrypt, &secret, Some(&secret)).unwrap();
          self.encryption_status = EncryptionStatus::Encrypted(crypter);
        }

        let response = EncryptionSuccessPacket {};
        self.sender.send_packet(response).await;

        self.state = State::Login;

        Ok(true)
      }
    }
  }
}

fn decrypt_header(crypter: &mut Crypter, data: [u8; 6]) -> [u8; 6] {
  let mut buf = [0; 6];
  let len = crypter.update(&data, &mut buf).unwrap();
  debug_assert_eq!(len, 6);
  buf
}

fn decrypt_vec(crypter: &mut Crypter, data: Vec<u8>) -> Vec<u8> {
  let mut buf = vec![0; data.len()];
  let len = crypter.update(&data, &mut buf).unwrap();
  debug_assert_eq!(len, buf.len());
  buf
}
