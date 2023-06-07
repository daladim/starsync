# starsync

An app to synchronize MTP devices with iTunes libraries.

## How to use

This app uses
* **sources** of music, playlists, ratings, etc.<br/>
  Currently, the only supported source is the local iTunes instance
* **devices** to sync content to, such as
  * connected MTP devices
  * every local disk (that aims at supporting syncing to SD cards, but one could also sync to `C:\`, even if that does not make much sense)
  * (for debugging purposes, in case the `debug_folder` Cargo feature is enabled) the `C:\Users\Public\Documents\` folder, slightly more convenient than the root of `C:`.

Run the app with the `starsync list-devices` or `starsync list-sources` command to list available devices or sources.

To be usable, a device must first be initialized (this boils down to creating a `starsync\` folder on its root and a default config file). This can be done with the `starsync init $device $source` command.<br/>
Initing a device ties it to the chosen source (this "tie" gets written in the config file into the device).

This folder can later be deleted by running `starsync deinit`.

Then, syncing a device with its source is as easy as `starsync sync $source`

## What is synced

This app will sync various things, depending on how a device is configured. This can be chosen by manually editing the config file on the device.

* selected playlists are synced.<br/>
  One of the valid playlists is "all the iTunes library". Its actual name depends on the current localization of iTunes.
* song ratings are synced, by creating 5 specific playlists for the 5 possible ratings.

Starsync can perform reverse sync, i.e. mirroring into the source the changes that have been performed on the device since the last sync. This includes
* playlist modifications (changes to the m3u files on the device)
* ratings modifications (changes to the ratings playlists)

In case changes have been performed on both the device and the source, Starsync will seamlessy merge them and apply them both ways.

## Android companion app

On Android, the Shuttle2 (S2) music app (or rather a fork of mine) is able to modify m3u playlist files whenever they are modified, and thus work out-of-the-box with Starsync.

