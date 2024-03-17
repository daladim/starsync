//! This code was autogenerated with `dbus-codegen-rust -d org.mpris.MediaPlayer2.rhythmbox -p /org/gnome/Rhythmbox3/PlaylistManager`, see https://github.com/diwic/dbus-rs
use dbus as dbus;
#[allow(unused_imports)]
use dbus::arg;
use dbus::blocking;

pub trait OrgFreedesktopDBusProperties {
    fn get(&self, interface_name: &str, property_name: &str) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error>;
    fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error>;
    fn set(&self, interface_name: &str, property_name: &str, value: arg::Variant<Box<dyn arg::RefArg>>) -> Result<(), dbus::Error>;
}

#[derive(Debug)]
pub struct OrgFreedesktopDBusPropertiesPropertiesChanged {
    pub interface_name: String,
    pub changed_properties: arg::PropMap,
    pub invalidated_properties: Vec<String>,
}

impl arg::AppendAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
    fn append(&self, i: &mut arg::IterAppend) {
        arg::RefArg::append(&self.interface_name, i);
        arg::RefArg::append(&self.changed_properties, i);
        arg::RefArg::append(&self.invalidated_properties, i);
    }
}

impl arg::ReadAll for OrgFreedesktopDBusPropertiesPropertiesChanged {
    fn read(i: &mut arg::Iter) -> Result<Self, arg::TypeMismatchError> {
        Ok(OrgFreedesktopDBusPropertiesPropertiesChanged {
            interface_name: i.read()?,
            changed_properties: i.read()?,
            invalidated_properties: i.read()?,
        })
    }
}

impl dbus::message::SignalArgs for OrgFreedesktopDBusPropertiesPropertiesChanged {
    const NAME: &'static str = "PropertiesChanged";
    const INTERFACE: &'static str = "org.freedesktop.DBus.Properties";
}

impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target=T>> OrgFreedesktopDBusProperties for blocking::Proxy<'a, C> {

    fn get(&self, interface_name: &str, property_name: &str) -> Result<arg::Variant<Box<dyn arg::RefArg + 'static>>, dbus::Error> {
        self.method_call("org.freedesktop.DBus.Properties", "Get", (interface_name, property_name, ))
            .and_then(|r: (arg::Variant<Box<dyn arg::RefArg + 'static>>, )| Ok(r.0, ))
    }

    fn get_all(&self, interface_name: &str) -> Result<arg::PropMap, dbus::Error> {
        self.method_call("org.freedesktop.DBus.Properties", "GetAll", (interface_name, ))
            .and_then(|r: (arg::PropMap, )| Ok(r.0, ))
    }

    fn set(&self, interface_name: &str, property_name: &str, value: arg::Variant<Box<dyn arg::RefArg>>) -> Result<(), dbus::Error> {
        self.method_call("org.freedesktop.DBus.Properties", "Set", (interface_name, property_name, value, ))
    }
}

pub trait OrgFreedesktopDBusIntrospectable {
    fn introspect(&self) -> Result<String, dbus::Error>;
}

impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target=T>> OrgFreedesktopDBusIntrospectable for blocking::Proxy<'a, C> {

    fn introspect(&self) -> Result<String, dbus::Error> {
        self.method_call("org.freedesktop.DBus.Introspectable", "Introspect", ())
            .and_then(|r: (String, )| Ok(r.0, ))
    }
}

pub trait OrgFreedesktopDBusPeer {
    fn ping(&self) -> Result<(), dbus::Error>;
    fn get_machine_id(&self) -> Result<String, dbus::Error>;
}

impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target=T>> OrgFreedesktopDBusPeer for blocking::Proxy<'a, C> {

    fn ping(&self) -> Result<(), dbus::Error> {
        self.method_call("org.freedesktop.DBus.Peer", "Ping", ())
    }

    fn get_machine_id(&self) -> Result<String, dbus::Error> {
        self.method_call("org.freedesktop.DBus.Peer", "GetMachineId", ())
            .and_then(|r: (String, )| Ok(r.0, ))
    }
}

pub trait OrgGnomeRhythmbox3PlaylistManager {
    fn get_playlists(&self) -> Result<Vec<String>, dbus::Error>;
    fn create_playlist(&self, name: &str) -> Result<(), dbus::Error>;
    fn delete_playlist(&self, name: &str) -> Result<(), dbus::Error>;
    fn add_to_playlist(&self, playlist: &str, uri: &str) -> Result<(), dbus::Error>;
    fn remove_from_playlist(&self, playlist: &str, uri: &str) -> Result<(), dbus::Error>;
    fn export_playlist(&self, playlist: &str, uri: &str, mp3_format: bool) -> Result<(), dbus::Error>;
    fn import_playlist(&self, uri: &str) -> Result<(), dbus::Error>;
}

impl<'a, T: blocking::BlockingSender, C: ::std::ops::Deref<Target=T>> OrgGnomeRhythmbox3PlaylistManager for blocking::Proxy<'a, C> {

    fn get_playlists(&self) -> Result<Vec<String>, dbus::Error> {
        self.method_call("org.gnome.Rhythmbox3.PlaylistManager", "GetPlaylists", ())
            .and_then(|r: (Vec<String>, )| Ok(r.0, ))
    }

    fn create_playlist(&self, name: &str) -> Result<(), dbus::Error> {
        self.method_call("org.gnome.Rhythmbox3.PlaylistManager", "CreatePlaylist", (name, ))
    }

    fn delete_playlist(&self, name: &str) -> Result<(), dbus::Error> {
        self.method_call("org.gnome.Rhythmbox3.PlaylistManager", "DeletePlaylist", (name, ))
    }

    fn add_to_playlist(&self, playlist: &str, uri: &str) -> Result<(), dbus::Error> {
        self.method_call("org.gnome.Rhythmbox3.PlaylistManager", "AddToPlaylist", (playlist, uri, ))
    }

    fn remove_from_playlist(&self, playlist: &str, uri: &str) -> Result<(), dbus::Error> {
        self.method_call("org.gnome.Rhythmbox3.PlaylistManager", "RemoveFromPlaylist", (playlist, uri, ))
    }

    fn export_playlist(&self, playlist: &str, uri: &str, mp3_format: bool) -> Result<(), dbus::Error> {
        self.method_call("org.gnome.Rhythmbox3.PlaylistManager", "ExportPlaylist", (playlist, uri, mp3_format, ))
    }

    fn import_playlist(&self, uri: &str) -> Result<(), dbus::Error> {
        self.method_call("org.gnome.Rhythmbox3.PlaylistManager", "ImportPlaylist", (uri, ))
    }
}
