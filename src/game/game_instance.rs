use tokio::sync::mpsc;
use tokio::task::JoinHandle;

// Structures

#[derive(Clone, Debug)]
pub struct GameHandle {
    pub id: u64,
    sender: mpsc::Sender<GameMessage>,
}

#[derive(Debug)]
pub struct GameActor {
    pub id: u64,
}

#[derive(Debug)]
enum GameMessage {
    StopActor,
}

// Implementations

impl GameHandle {
    pub async fn stop_actor(&mut self) {
        self.sender.send(GameMessage::StopActor)
            .await
            .expect("GameActor was dropped, oopsie!")
    }
}

impl GameActor {
    pub fn spawn(self) -> (GameHandle, JoinHandle<GameActor>) {
        let (send, recv) = mpsc::channel(1024);

        (
            GameHandle {
                id: self.id,
                sender: send,
            },
            tokio::spawn(async move {
                self.actor(recv).await
            })
        )
    }
    async fn actor(self, mut recv: mpsc::Receiver<GameMessage>) -> Self {
        loop {
            match recv.recv().await {
                None => return self,
                Some(GameMessage::StopActor) => return self
            }
        }
    }
}