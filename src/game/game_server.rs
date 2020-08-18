use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use openssl::rsa::Rsa;

use super::*;

// Structures

#[derive(Clone, Debug)]
pub struct GameServerHandle {
    address: SocketAddr,
    sender: mpsc::Sender<GameServerMessage>,
}

#[derive(Debug)]
pub struct GameServerActor {
    address: SocketAddr,
    /// Shared immutable reference to the public and private key.
    /// The public key is also stored using DER.
    encryption: Arc<(Rsa<openssl::pkey::Private>, Vec<u8>)>,
    /// Shared mutable HashMap containing all active connections.
    connections: Arc<Mutex<HashMap<SocketAddr, net::NetManagerHandle>>>,
    games: HashMap<u64, GameHandle>,
}

#[derive(Debug)]
enum GameServerMessage {
    StopActor,
}

// Implementations

impl GameServerHandle {
    pub async fn stop_actor(&mut self) {
        self.sender
            .send(GameServerMessage::StopActor)
            .await
            .expect("GameServerActor was dropped, oopsie!")
    }
}

impl GameServerActor {
    pub fn new<A: Into<SocketAddr>>(addr: A) -> Self {
        // Generate a 2048-bit Rsa key pair
        let rsa = Rsa::generate(2048).unwrap();
        // Prepare DER representation
        let der = rsa.public_key_to_der().unwrap();
        Self {
            address: addr.into(),
            encryption: (rsa, der.into()).into(),
            games: HashMap::new(),
            connections: Mutex::new(HashMap::new()).into(),
        }
    }
    pub fn spawn(self) -> (GameServerHandle, JoinHandle<GameServerActor>) {
        let (send, recv) = mpsc::channel(1024);

        (
            GameServerHandle {
                sender: send,
                address: self.address,
            },
            tokio::spawn(async move { self.actor(recv).await }),
        )
    }
    async fn actor(mut self, mut recv: mpsc::Receiver<GameServerMessage>) -> Self {
        use futures::FutureExt;

        let mut net_listener = TcpListener::bind(self.address)
            .await
            .expect("Failed to register server");
        println!("(ℹ) Server listening on {}", self.address);

        loop {
            futures::select! {
                msg = recv.recv().fuse() => {
                    match msg {
                        None => return self.stop_net().await,
                        Some(GameServerMessage::StopActor) => return self.stop_net().await,
                    }
                },
                con_res = net_listener.accept().fuse() => {
                    match con_res {
                        Err(e) => {
                            println!("Failed to accept connection");
                            continue;
                        }
                        Ok((stream, address)) => {
                            self.accept_net(stream, address);
                            continue;
                        }
                    }
                }
            }
        }
    }
    /// Stops this actor after closing all network connections
    async fn stop_net(self) -> Self {
        let mut cons_lock = self.connections.lock().await;
        let cons_len = cons_lock.len();

        // Aquire ownership of the HashMap by swapping it with an empty one
        // The HashMap will be emptied anyways and ownership is required
        // to destructively iterate over that map.
        let cons = std::mem::replace(&mut *cons_lock, HashMap::with_capacity(0));
        // Drop the Mutex lock here because we don't need to access the
        // just swapped map anymore
        drop(cons_lock);

        let mut tasks = Vec::with_capacity(cons_len);
        for (_, mut connection) in cons {
            tasks.push(tokio::task::spawn(async move {
                connection.stop_actor().await;
            }));
        }

        // Wait for all connections to be terminated safely
        for jh in tasks {
            // We actually don't care if it really succeeded
            // It's just important that *there was an attempt*
            drop(jh.await);
        }

        self
    }
    /// Accepts a network connection socket
    fn accept_net(&mut self, stream: TcpStream, addr: SocketAddr) {
        println!("(ℹ) [+] Connection from {}", addr);
        let actor = net::NetManagerActor::new(addr, stream, self.encryption.clone());
        let (handle, jh) = actor.spawn();

        let cons_mutex = self.connections.clone();

        // Insert handle and wait until disconnect to remove it
        // We're going to wait a lot here, so green threads (tasks)
        // are much more efficient.
        tokio::task::spawn(async move {
            let mut lock = cons_mutex.lock().await;
            lock.insert(addr, handle);
            drop(lock);

            match jh.await {
                Ok(_) => println!("(ℹ) [-] {address} disconnected", address = addr),
                Err(err) => println!(
                    "(⚠) [-] {address} disconnected with an error:\n{error}",
                    address = addr,
                    error = err
                ),
            }

            let mut lock = cons_mutex.lock().await;
            lock.remove(&addr);
            drop(lock);
        });
    }
}
