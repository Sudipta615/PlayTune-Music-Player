#![allow(clippy::too_many_arguments)]

pub mod albums_artists;
pub mod audio_features;
pub mod cover_art;
pub mod database;
pub mod folders;
pub mod models;
pub mod playlists;
pub mod ratings;
pub mod schema;
pub mod settings;
pub mod tracks;

pub use database::*;
pub use models::*;
