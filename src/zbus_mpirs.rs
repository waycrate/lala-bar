//use futures_util::StreamExt;
use futures_util::StreamExt;
use once_cell::sync::Lazy;

use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use zbus::fdo::DBusProxy;
use zbus::zvariant::OwnedObjectPath;
use zbus::{dbus_proxy, zvariant::OwnedValue, Result};

use std::sync::OnceLock;
#[allow(unused)]
#[derive(Debug, Clone)]
pub struct Metadata {
    mpris_trackid: OwnedObjectPath,
    mpris_arturl: String,
    pub xesam_title: String,
    xesam_album: String,
    xesam_artist: Vec<String>,
}

impl Metadata {
    fn from_hashmap(value: &HashMap<String, OwnedValue>) -> Self {
        let art_url = &value.get("mpris:artUrl");
        let mut mpris_arturl = String::new();
        if let Some(art_url) = art_url {
            mpris_arturl = (*art_url).clone().try_into().unwrap_or_default();
        }

        let trackid = &value["mpris:trackid"];
        let mpris_trackid: OwnedObjectPath = trackid.clone().try_into().unwrap_or_default();

        let title = &value["xesam:title"];
        let xesam_title: String = title.clone().try_into().unwrap_or_default();

        let artist = &value["xesam:artist"];
        let xesam_artist: Vec<String> = artist.clone().try_into().unwrap_or_default();

        let album = &value["xesam:album"];
        let xesam_album: String = album.clone().try_into().unwrap_or_default();

        Self {
            mpris_trackid,
            xesam_title,
            xesam_artist,
            xesam_album,
            mpris_arturl,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ServiceInfo {
    service_path: String,
    pub can_play: bool,
    pub can_pause: bool,
    pub playback_status: String,
    pub metadata: Metadata,
}

impl ServiceInfo {
    fn new(
        path: &str,
        can_play: bool,
        can_pause: bool,
        playback_status: String,
        value: &HashMap<String, OwnedValue>,
    ) -> Self {
        Self {
            service_path: path.to_owned(),
            can_play,
            can_pause,
            playback_status,
            metadata: Metadata::from_hashmap(value),
        }
    }

    pub async fn pause(&self) -> Result<()> {
        let conn = get_connection().await?;
        let instance = MediaPlayer2DbusProxy::builder(&conn)
            .destination(self.service_path.as_str())?
            .build()
            .await?;
        instance.pause().await?;
        Ok(())
    }

    pub async fn play(&self) -> Result<()> {
        let conn = get_connection().await?;
        let instance = MediaPlayer2DbusProxy::builder(&conn)
            .destination(self.service_path.as_str())?
            .build()
            .await?;
        instance.play().await?;
        Ok(())
    }
}

static SESSION: OnceLock<zbus::Connection> = OnceLock::new();

async fn get_connection() -> zbus::Result<zbus::Connection> {
    if let Some(cnx) = SESSION.get() {
        Ok(cnx.clone())
    } else {
        let cnx = zbus::Connection::session().await?;
        SESSION.set(cnx.clone()).expect("Can't reset a OnceCell");
        Ok(cnx)
    }
}

pub static MPIRS_CONNECTIONS: Lazy<Arc<Mutex<Vec<ServiceInfo>>>> =
    Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

async fn mpirs_is_ready_in<T: ToString>(path: T) -> bool {
    let conns = MPIRS_CONNECTIONS.lock().await;
    conns
        .iter()
        .any(|info| info.service_path == path.to_string())
}

async fn set_mpirs_connection(list: Vec<ServiceInfo>) {
    let mut conns = MPIRS_CONNECTIONS.lock().await;
    *conns = list;
}

async fn add_mpirs_connection(mpirs_service_info: ServiceInfo) -> Result<()> {
    let mut conns = MPIRS_CONNECTIONS.lock().await;
    conns.push(mpirs_service_info.clone());
    drop(conns);
    let service_path = mpirs_service_info.service_path.clone();
    let service_path2 = mpirs_service_info.service_path.clone();
    let conn = get_connection().await?;
    let instance = MediaPlayer2DbusProxy::builder(&conn)
        .destination(service_path.clone())?
        .build()
        .await?;
    let mut statuschanged = instance.receive_playback_status_changed().await;
    tokio::spawn(async move {
        while let Some(signal) = statuschanged.next().await {
            let status: String = signal.get().await?;
            let mut conns = MPIRS_CONNECTIONS.lock().await;
            if let Some(index) = conns
                .iter()
                .position(|info| info.service_path == service_path.clone())
            {
                conns[index].playback_status = status;
            } else {
                break;
            }
        }
        Ok::<(), anyhow::Error>(())
    });
    let mut metadatachanged = instance.receive_metadata_changed().await;
    tokio::spawn(async move {
        while let Some(signal) = metadatachanged.next().await {
            let metadatamap = signal.get().await?;
            let metadata = Metadata::from_hashmap(&metadatamap);
            let mut conns = MPIRS_CONNECTIONS.lock().await;
            if let Some(index) = conns
                .iter()
                .position(|info| info.service_path == service_path2.clone())
            {
                conns[index].metadata = metadata;
            } else {
                break;
            }
        }
        Ok::<(), anyhow::Error>(())
    });
    Ok(())
}

async fn remove_mpirs_connection<T: ToString>(conn: T) {
    let mut conns = MPIRS_CONNECTIONS.lock().await;
    conns.retain(|iter| iter.service_path != conn.to_string());
}

#[dbus_proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait MediaPlayer2Dbus {
    #[dbus_proxy(property)]
    fn can_pause(&self) -> Result<bool>;

    #[dbus_proxy(property)]
    fn playback_status(&self) -> Result<String>;

    #[dbus_proxy(property)]
    fn can_play(&self) -> Result<bool>;

    #[dbus_proxy(property)]
    fn can_go_next(&self) -> Result<bool>;

    #[dbus_proxy(property)]
    fn can_go_previous(&self) -> Result<bool>;

    #[dbus_proxy(property)]
    fn metadata(&self) -> Result<HashMap<String, OwnedValue>>;

    fn pause(&self) -> Result<()>;

    fn play(&self) -> Result<()>;
}

pub async fn init_pris() -> Result<()> {
    let conn = get_connection().await?;
    let freedesktop = DBusProxy::new(&conn).await?;
    let names = freedesktop.list_names().await?;
    let names: Vec<String> = names
        .iter()
        .filter(|name| name.starts_with("org.mpris.MediaPlayer2"))
        .cloned()
        .map(|name| name.to_string())
        .collect();

    let mut serviceinfos = Vec::new();
    for name in names.iter() {
        let instance = MediaPlayer2DbusProxy::builder(&conn)
            .destination(name.as_str())?
            .build()
            .await?;

        let value = instance.metadata().await?;
        let can_pause = instance.can_pause().await?;
        let can_play = instance.can_play().await?;
        let playback_status = instance.playback_status().await?;
        serviceinfos.push(ServiceInfo::new(
            name,
            can_play,
            can_pause,
            playback_status,
            &value,
        ));
    }

    set_mpirs_connection(serviceinfos).await;
    tokio::spawn(async move {
        let mut namechangesignal = freedesktop.receive_name_owner_changed().await?;
        while let Some(signal) = namechangesignal.next().await {
            let (interfacename, added, removed): (String, String, String) = signal.body()?;
            if !interfacename.starts_with("org.mpris.MediaPlayer2") {
                continue;
            }
            if removed.is_empty() {
                remove_mpirs_connection(&interfacename).await;
            } else if added.is_empty() && !mpirs_is_ready_in(interfacename.as_str()).await {
                let instance = MediaPlayer2DbusProxy::builder(&conn)
                    .destination(interfacename.as_str())?
                    .build()
                    .await?;

                let value = instance.metadata().await?;
                let can_pause = instance.can_pause().await?;
                let can_play = instance.can_play().await?;
                let playback_status = instance.playback_status().await?;
                add_mpirs_connection(ServiceInfo::new(
                    interfacename.as_str(),
                    can_play,
                    can_pause,
                    playback_status,
                    &value,
                ))
                .await
                .ok();
            }
            //println!("name: {:?}", get_mpirs_connections().await);
        }
        Ok::<(), anyhow::Error>(())
    });
    Ok(())
}
