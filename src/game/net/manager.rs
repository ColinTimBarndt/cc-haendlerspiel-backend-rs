use std::fmt;
use std::net::SocketAddr;

use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use tokio_rustls::TlsAcceptor;

use crate::game::GameServerHandle;

// Structures

#[derive(Clone, Debug)]
pub struct NetManagerHandle {
    pub address: SocketAddr,
    sender: mpsc::Sender<NetManagerMessage>,
}

pub struct NetManagerActor {
    pub address: SocketAddr,
    pub stream: Option<TcpStream>,
    tls_acceptor: Option<TlsAcceptor>,
    server: GameServerHandle,
}

#[derive(Debug)]
enum NetManagerMessage {
    StopActor,
}

// Implementations

impl fmt::Debug for NetManagerActor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetManagerActor")
            .field("address", &self.address)
            .field("stream", &self.stream)
            .field("tls_acceptor", &self.tls_acceptor.as_ref().map(|_| "<...>"))
            .finish()
    }
}

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
        tls_acceptor: TlsAcceptor,
        gs_handle: GameServerHandle,
    ) -> Self {
        Self {
            address,
            stream: Some(stream),
            tls_acceptor: Some(tls_acceptor),
            server: gs_handle,
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

        // Establish TLS
        let acceptor = std::mem::replace(&mut self.tls_acceptor, None).unwrap();
        let stream = match acceptor.accept(stream).await {
            Err(e) => {
                eprintln!(
                    "(⚠) TLS handshake with {addr} failed: {err}",
                    addr = self.address,
                    err = e
                );
                return self;
            }
            Ok(s) => s,
        };

        // Split into actors
        let (rh, wh) = tokio::io::split(stream);

        // Spawn Send Actor
        let send_actor = super::sender::NetSenderActor::new(wh, self.address.clone());
        let (mut send_handle, send_jh) = send_actor.spawn();

        // Spawn Receive Actor
        let recv_actor = super::receiver::NetReceiverActor::new(
            rh,
            send_handle.clone(),
            self.address.clone(),
            self.server.clone(),
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

        // Shutdown connection
        let (rh, wh): (tokio::io::ReadHalf<_>, _) =
            (recv_actor.unwrap().into(), send_actor.unwrap().into());
        let (stream, _session) = rh.unsplit(wh).into_inner();
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
