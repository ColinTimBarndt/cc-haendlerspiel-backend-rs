use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use std::collections::HashSet;
use std::sync::Arc;

use crate::game::net::NetSenderHandle;

// Structures

#[derive(Clone, Debug)]
pub struct GameHandle {
    pub info: Arc<GameInfo>,
    sender: mpsc::Sender<GameMessage>,
}

#[derive(Debug)]
pub struct GameActor {
    pub info: Arc<GameInfo>,
    pub players: HashSet<NetSenderHandle>,
}

#[derive(Debug)]
enum GameMessage {
    StopActor,
    GetPlayerCount(oneshot::Sender<usize>),
}

#[derive(Debug)]
pub struct GameInfo {
    pub id: u64,
    pub name: String,
}

// Implementations

const ACTOR_DROPPED_ERROR: &'static str = "GameActor was dropped, oopsie!";

impl GameHandle {
    pub async fn stop_actor(&mut self) {
        self.sender
            .send(GameMessage::StopActor)
            .await
            .expect(ACTOR_DROPPED_ERROR)
    }
    pub async fn get_player_count(&mut self) -> usize {
        let (send, recv) = oneshot::channel();
        self.sender
            .send(GameMessage::GetPlayerCount(send))
            .await
            .expect(ACTOR_DROPPED_ERROR);
        recv.await.expect(ACTOR_DROPPED_ERROR)
    }
}

impl GameActor {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            info: GameInfo { id, name }.into(),
            players: HashSet::new(),
        }
    }
    pub fn spawn(self) -> (GameHandle, JoinHandle<GameActor>) {
        let (send, recv) = mpsc::channel(1024);

        (
            GameHandle {
                info: self.info.clone(),
                sender: send,
            },
            tokio::spawn(async move { self.actor(recv).await }),
        )
    }
    async fn actor(self, mut recv: mpsc::Receiver<GameMessage>) -> Self {
        loop {
            match recv.recv().await {
                None => return self,
                Some(GameMessage::StopActor) => return self,
                Some(GameMessage::GetPlayerCount(cb)) => {
                    drop(cb.send(self.players.len()));
                    continue;
                }
            }
        }
    }
}
