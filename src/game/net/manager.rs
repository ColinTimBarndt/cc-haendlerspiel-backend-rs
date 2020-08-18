use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use openssl::rsa::Rsa;

// Structures

#[derive(Clone, Debug)]
pub struct NetManagerHandle {
    pub address: SocketAddr,
    sender: mpsc::Sender<NetManagerMessage>,
}

#[derive(Debug)]
pub struct NetManagerActor {
    pub address: SocketAddr,
    pub stream: Option<TcpStream>,
    encryption: Arc<(Rsa<openssl::pkey::Private>, Vec<u8>)>,
}

#[derive(Debug)]
enum NetManagerMessage {
    StopActor,
}

// Implementations

impl NetManagerHandle {
    pub async fn stop_actor(&mut self) {
        self.sender
            .send(NetManagerMessage::StopActor)
            .await
            .expect("NetManagerActor was dropped, oopsie!")
    }
}

impl NetManagerActor {
    pub fn new(
        address: SocketAddr,
        stream: TcpStream,
        encryption: Arc<(Rsa<openssl::pkey::Private>, Vec<u8>)>,
    ) -> Self {
        Self {
            address,
            stream: Some(stream),
            encryption,
        }
    }
    pub fn spawn(self) -> (NetManagerHandle, JoinHandle<NetManagerActor>) {
        let (send, recv) = mpsc::channel(1024);

        (
            NetManagerHandle {
                address: self.address,
                sender: send,
            },
            tokio::spawn(async move { self.actor(recv).await }),
        )
    }
    async fn actor(mut self, mut recv: mpsc::Receiver<NetManagerMessage>) -> Self {
        use futures::future::FutureExt;
        let stream = std::mem::replace(&mut self.stream, None).unwrap();

        let (rh, wh) = stream.into_split();

        // Spawn Send Actor
        let send_actor = super::sender::NetSenderActor::new(wh);
        let (mut send_handle, send_jh) = send_actor.spawn();

        // Spawn Receive Actor
        let recv_actor = super::receiver::NetReceiverActor::new(
            rh,
            self.encryption.clone(),
            send_handle.clone(),
            self.address.clone(),
        );
        let (mut recv_handle, recv_jh) = recv_actor.spawn();

        tokio::pin! {
            let recv_finished = recv_jh;
            let send_finished = send_jh;
        }

        let mut recv_actor = None;
        let mut send_actor = None;

        loop {
            tokio::select! {
                msg = recv.recv().fuse() => {
                    if let Some(msg) = msg {
                        if !self.process_msg(msg).await {
                            recv_handle.stop_actor().await;
                            send_handle.stop_actor().await;
                            break;
                        }
                    } else {
                        recv_handle.stop_actor().await;
                        send_handle.stop_actor().await;
                        let res = tokio::join!(recv_finished, send_finished);
                        recv_actor = Some(res.0.unwrap());
                        send_actor = Some(res.1.unwrap());
                        break;
                    }
                }
                act = &mut recv_finished => {
                    send_handle.stop_actor().await;
                    recv_actor = Some(act.unwrap());
                    send_actor = Some(send_finished.await.unwrap());
                    break;
                }
                act = &mut send_finished => {
                    recv_handle.stop_actor().await;
                    recv_actor = Some(recv_finished.await.unwrap());
                    send_actor = Some(act.unwrap());
                    break;
                }
            }
        }

        let (rh, wh) = (
            recv_actor.unwrap().read_half.into_inner(),
            send_actor.unwrap().write_half,
        );
        let stream = rh.reunite(wh).unwrap();
        drop(stream.shutdown(std::net::Shutdown::Both));
        self.stream = Some(stream);

        self
    }

    async fn process_msg(&mut self, msg: NetManagerMessage) -> bool {
        match msg {
            NetManagerMessage::StopActor => return false,
            //_ => true,
        }
    }
}
