use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::net::tcp::OwnedWriteHalf;

use super::packet::OutgoingPacket;
use super::packet::serial::SerialWrite;

// Structures

#[derive(Clone, Debug)]
pub struct NetSenderHandle {
    sender: mpsc::Sender<NetSenderMessage>,
}

#[derive(Debug)]
pub struct NetSenderActor {
    pub write_half: OwnedWriteHalf,
}

#[derive(Debug)]
enum NetSenderMessage {
    StopActor,
    SendPacket(Vec<u8>),
}

// Implementations

const ACTOR_DROPPED_ERROR: &'static str = "NetSenderActor was dropped, oopsie!";

impl NetSenderHandle {
    pub async fn stop_actor(&mut self) {
        self.sender.send(NetSenderMessage::StopActor)
            .await
            .expect(ACTOR_DROPPED_ERROR)
    }
    pub async fn send_packet<P: OutgoingPacket>(&mut self, packet: P) {
        let mut buf = Vec::with_capacity(40);
        SerialWrite::write_consume(P::ID, &mut buf);

        let mut pbuf = Vec::with_capacity(32);
        SerialWrite::write_consume(packet, &mut pbuf);

        SerialWrite::write_consume(
            std::convert::TryInto::<u32>::try_into(pbuf.len())
                .expect("Packet is too large!"),
            &mut buf
        );

        self.sender.send(NetSenderMessage::SendPacket(buf))
            .await
            .expect(ACTOR_DROPPED_ERROR)
    }
}

impl NetSenderActor {
    pub fn new(write_half: OwnedWriteHalf) -> Self {
        Self {
            write_half
        }
    }
    pub fn spawn(self) -> (NetSenderHandle, JoinHandle<NetSenderActor>) {
        let (send, recv) = mpsc::channel(1024);

        (
            NetSenderHandle {
                sender: send,
            },
            tokio::spawn(async move {
                self.actor(recv).await
            })
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
                }
            }
        }
    }
}