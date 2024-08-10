//! # D-Bus interface proxy for: `org.freedesktop.Notifications`
//!
//! This code was generated by `zbus-xmlgen` `4.1.0` from D-Bus introspection data.
//! Source: `Interface '/org/freedesktop/Notifications' from service 'org.freedesktop.Notifications' on system bus`.
//!
//! You may prefer to adapt it, instead of using it verbatim.
//!
//! More information can be found in the [Writing a client proxy] section of the zbus
//! documentation.
//!
//! This type implements the [D-Bus standard interfaces], (`org.freedesktop.DBus.*`) for which the
//! following zbus API can be used:
//!
//! * [`zbus::fdo::PeerProxy`]
//! * [`zbus::fdo::IntrospectableProxy`]
//! * [`zbus::fdo::PropertiesProxy`]
//!
//! Consequently `zbus-xmlgen` did not generate code for the above interfaces.
//!
//! [Writing a client proxy]: https://dbus2.github.io/zbus/client.html
//! [D-Bus standard interfaces]: https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces,
use zbus::{interface, object_server::SignalContext, zvariant::OwnedValue};

use futures::{channel::mpsc::Sender, never::Never};
use zbus::ConnectionBuilder;

use std::future::pending;

#[allow(unused)]
const NOTIFICATION_DELETED_BY_EXPIRED: u32 = 1;
const NOTIFICATION_DELETED_BY_USER: u32 = 2;

#[allow(unused)]
const NOTIFICATION_CLOSED_BY_DBUS: u32 = 3;
#[allow(unused)]
const NOTIFICATION_CLOSED_BY_UNKNOWN_REASON: u32 = 4;

#[derive(Debug, Clone)]
pub enum NotifyMessage {
    UnitAdd(NotifyUnit),
    UnitRemove(u32),
}

#[derive(Debug, Clone)]
pub struct NotifyUnit {
    pub app_name: String,
    pub id: u32,
    pub icon: String,
    pub summery: String,
    pub body: String,
    pub actions: Vec<String>,
    pub timeout: i32,
}

#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub spec_version: String,
}

#[derive(Debug)]
pub struct LaLaMako<T: From<NotifyMessage> + Send> {
    capabilities: Vec<String>,
    sender: Sender<T>,
    version: VersionInfo,
}

#[interface(name = "org.freedesktop.Notifications")]
impl<T: From<NotifyMessage> + Send + 'static> LaLaMako<T> {
    // CloseNotification method
    async fn close_notification(
        &mut self,
        #[zbus(signal_context)] ctx: SignalContext<'_>,
        id: u32,
    ) -> zbus::fdo::Result<()> {
        self.notification_closed(&ctx, id, NOTIFICATION_DELETED_BY_USER)
            .await
            .ok();
        self.sender
            .try_send(NotifyMessage::UnitRemove(id).into())
            .ok();
        Ok(())
    }

    /// GetCapabilities method
    fn get_capabilities(&self) -> Vec<String> {
        self.capabilities.clone()
    }

    /// GetServerInformation method
    fn get_server_information(&self) -> (String, String, String, String) {
        let VersionInfo {
            name,
            vendor,
            version,
            spec_version,
        } = &self.version;
        (
            name.clone(),
            vendor.clone(),
            version.clone(),
            spec_version.clone(),
        )
    }

    // Notify method
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &mut self,
        app_name: &str,
        id: u32,
        icon: &str,
        summery: &str,
        body: &str,
        actions: Vec<&str>,
        _hints: std::collections::HashMap<&str, OwnedValue>,
        timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        self.sender
            .try_send(
                NotifyMessage::UnitAdd(NotifyUnit {
                    app_name: app_name.to_string(),
                    id,
                    icon: icon.to_string(),
                    summery: summery.to_string(),
                    body: body.to_string(),
                    actions: actions.iter().map(|a| a.to_string()).collect(),
                    timeout,
                })
                .into(),
            )
            .ok();
        Ok(0)
    }

    #[zbus(signal)]
    async fn action_invoked(
        &self,
        ctx: &SignalContext<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    // NotificationClosed signal
    #[zbus(signal)]
    async fn notification_closed(
        &self,
        ctx: &SignalContext<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;
}

pub async fn start_server<T: From<NotifyMessage> + Send + 'static>(
    sender: Sender<T>,
    capabilities: Vec<String>,
    version: VersionInfo,
) -> Never {
    let _conn = async {
        ConnectionBuilder::session()?
            .name("org.freedesktop.Notifications")?
            .serve_at(
                "/org/freedesktop/Notifications",
                LaLaMako {
                    sender,
                    capabilities,
                    version,
                },
            )?
            .build()
            .await
    }
    .await;

    pending::<()>().await;

    unreachable!()
}
