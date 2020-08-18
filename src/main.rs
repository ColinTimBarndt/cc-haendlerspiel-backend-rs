use std::error::Error;

pub mod game;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let game_server = game::GameServerActor::new(([127, 0, 0, 1], 25252));
    let (mut handle, join_handle) = game_server.spawn();

    wait_ctrl_c_signal().await;
    handle.stop_actor().await;
    join_handle.await?;

    println!("Goodbye!");
    Ok(())
}

async fn wait_ctrl_c_signal() {
    // Wait for the CTRL+C signal
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install CTRL+C signal handler");
    println!("Shutting down server..");
}
