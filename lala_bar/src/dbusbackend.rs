use futures::channel::mpsc::Sender;

use crate::Message;
use zbus::{connection, interface};
pub struct LalaBarBackend {
    sender: Sender<Message>,
}

#[interface(name = "org.lalabar.Backend")]
impl LalaBarBackend {
    fn toggle_bar(&mut self) {
        self.sender.try_send(Message::ToggleLauncherDBus).ok();
    }
}

pub async fn start_backend(sender: Sender<Message>) -> Result<zbus::Connection, zbus::Error> {
    connection::Builder::session()?
        .name("org.lalabar.backend")?
        .serve_at("/org/lalabar/Backend", LalaBarBackend { sender })?
        .build()
        .await
}
