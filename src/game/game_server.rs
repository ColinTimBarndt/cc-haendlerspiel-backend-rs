use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

use tokio_rustls::{
    rustls::{Certificate, PrivateKey, ServerConfig},
    TlsAcceptor,
};

use super::*;

// Structures

#[derive(Clone, Debug)]
pub struct GameServerHandle {
    address: SocketAddr,
    sender: mpsc::Sender<GameServerMessage>,
}

pub struct GameServerActor {
    address: SocketAddr,
    tls_acceptor: TlsAcceptor,
    /// Shared mutable HashMap containing all active connections.
    connections: Arc<Mutex<HashMap<SocketAddr, net::NetManagerHandle>>>,
    games: HashMap<u64, GameHandle>,
}

#[derive(Debug)]
enum GameServerMessage {
    StopActor,
    GetGames(oneshot::Sender<Vec<(u64, GameHandle)>>),
}

// Implementations

const ACTOR_DROPPED_MESSAGE: &'static str = "GameServerActor was dropped, oopsie!";

impl GameServerHandle {
    pub async fn stop_actor(&mut self) {
        self.sender
            .send(GameServerMessage::StopActor)
            .await
            .expect(ACTOR_DROPPED_MESSAGE)
    }
    pub async fn get_games(&mut self) -> Vec<(u64, GameHandle)> {
        let (send, recv) = oneshot::channel();
        self.sender
            .send(GameServerMessage::GetGames(send))
            .await
            .expect(ACTOR_DROPPED_MESSAGE);
        recv.await.expect(ACTOR_DROPPED_MESSAGE)
    }
}

impl std::fmt::Debug for GameServerActor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GameServerActor")
            .field("address", &self.address)
            .field("connections", &self.connections)
            .field("games", &self.games)
            .field("tls_acceptor", &"<...>")
            .finish()
    }
}

impl GameServerActor {
    pub fn new<A: Into<SocketAddr>>(addr: A, encryption: (Vec<Certificate>, PrivateKey)) -> Self {
        let mut tls_config = ServerConfig::new(tokio_rustls::rustls::NoClientAuth::new().into());
        tls_config
            .set_single_cert(encryption.0, encryption.1)
            .unwrap();
        Self {
            address: addr.into(),
            tls_acceptor: TlsAcceptor::from(Arc::from(tls_config)),
            games: HashMap::new(),
            connections: Mutex::new(HashMap::new()).into(),
        }
    }
    pub fn spawn(self) -> (GameServerHandle, JoinHandle<GameServerActor>) {
        let (send, recv) = mpsc::channel(1024);
        let handle = GameServerHandle {
            sender: send,
            address: self.address,
        };

        (
            handle.clone(),
            tokio::spawn(async move { self.actor(recv, handle).await }),
        )
    }
    async fn actor(
        mut self,
        mut recv: mpsc::Receiver<GameServerMessage>,
        game_server_handle: GameServerHandle,
    ) -> Self {
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
                        Some(msg) => if !self.process_msg(msg) {
                            return self.stop_net().await
                        },
                    }
                },
                con_res = net_listener.accept().fuse() => {
                    match con_res {
                        Err(e) => {
                            println!("Failed to accept connection");
                            continue;
                        }
                        Ok((stream, address)) => {
                            self.accept_net(stream, address, game_server_handle.clone());
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
    fn accept_net(
        &mut self,
        stream: TcpStream,
        addr: SocketAddr,
        game_server_handle: GameServerHandle,
    ) {
        println!("(ℹ) [+] Connection from {}", addr);
        let actor =
            net::NetManagerActor::new(addr, stream, self.tls_acceptor.clone(), game_server_handle);
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
    /// Processes an actor message and returns if the actor should continue listening
    fn process_msg(&mut self, msg: GameServerMessage) -> bool {
        match msg {
            GameServerMessage::StopActor => false,
            GameServerMessage::GetGames(cb) => {
                drop(
                    cb.send(
                        self.games
                            .iter()
                            .map(|(id, gh)| (*id, gh.clone()))
                            .collect(),
                    ),
                );
                true
            }
        }
    }
}
