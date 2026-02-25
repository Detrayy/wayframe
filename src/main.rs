mod config;
mod server;
mod types;
mod ui;

use config::keyboard::keyboard_config_from_system;
use crossbeam_channel::unbounded;
use smithay::reexports::wayland_server::ListeningSocket;
use std::env;
use std::process::Command;
use std::thread;
use types::{GtkToServerMsg, ServerToGtkMsg};

fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting WayFrame...");

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <command> [args...]", args[0]);
        std::process::exit(1);
    }

    let target_app = &args[1];
    let app_args = &args[2..];
    let keyboard_config = keyboard_config_from_system();

    let (server_tx, gtk_rx) = unbounded::<ServerToGtkMsg>();
    let (gtk_tx, server_rx) = unbounded::<GtkToServerMsg>();

    let socket = ListeningSocket::bind_auto("wayland", 0..30).expect("Failed to bind socket");
    let socket_name = socket
        .socket_name()
        .expect("Socket name should exist for auto-bound socket")
        .to_string_lossy()
        .into_owned();

    let socket_name_clone = socket_name.clone();
    thread::spawn(move || {
        server::run_server(
            socket,
            socket_name_clone,
            server_rx,
            server_tx,
            keyboard_config,
        );
    });

    tracing::info!("Launching target app: {}", target_app);
    let mut child = Command::new(target_app)
        .args(app_args)
        .env("WAYLAND_DISPLAY", &socket_name)
        .env("OZONE_PLATFORM", "wayland")
        .spawn()
        .expect("Failed to launch app");

    ui::run_ui(gtk_tx, gtk_rx, target_app.clone());

    let _ = child.kill();
}
