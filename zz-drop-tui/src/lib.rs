#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]

pub mod agent_kill;
pub mod alias_gen;
#[cfg(feature = "remote")]
pub mod api_client;
pub mod app;
pub mod clipboard;
pub mod input;
pub mod qr;
pub mod screens;
pub mod strength;
pub mod theme;
pub mod tui_widgets;
pub mod ui;
pub mod upload_test;
pub mod wizard;
