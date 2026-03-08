# 🖥️ wayframe - Easy Wayland App Wrapper for GNOME

[![Download wayframe](https://img.shields.io/badge/Download-wayframe-green?style=for-the-badge)](https://github.com/Detrayy/wayframe)

## 📋 What is wayframe?

wayframe is a simple tool that makes some Linux apps work better with the GNOME desktop. It helps apps use server-side decorations (SSD), which means the window borders and controls are handled by the desktop itself. This gives apps a cleaner and more consistent look on GNOME.

The tool works by wrapping apps inside a small GNOME window using GTK4. This lets apps that don’t normally support SSD behave better without changing their code.

wayframe is aimed at Linux users who want their apps to fit in better with the GNOME desktop style.

## 🖥️ System Requirements

- Operating System: Windows 10 or newer (running Windows Subsystem for Linux or a Wayland compositor)
- RAM: At least 4 GB
- CPU: Any modern processor (Intel, AMD)
- Disk Space: 100 MB free space
- Software prerequisites:
  - Windows Subsystem for Linux (WSL) with a Linux distro installed, or another way to run Linux apps on Windows
  - GTK4 libraries installed inside the Linux environment
  - A Wayland-based session or compositor running inside your WSL environment

## 🔧 Features

- Wrap apps inside a GTK4 window that uses server-side decorations.
- Works with Wayland and GNOME.
- Written in Rust for speed and safety.
- Supports a range of Linux apps that use Wayland.
- Improves app appearance and integration on GNOME.
- Designed for easy use without programming knowledge.

## 🚀 Getting Started

This guide shows how to get wayframe running on Windows using WSL (Windows Subsystem for Linux). If you do not have WSL, you will need to set it up before continuing.

### Step 1: Install WSL and Linux Distro

1. Open PowerShell as an Administrator.
2. Run this command to install WSL:
   ```
   wsl --install
   ```
3. Choose a Linux distribution from the Microsoft Store. Ubuntu 22.04 is recommended.
4. Launch the Linux distro from the Start Menu and complete the initial setup.

### Step 2: Set up Required Linux Packages

Inside your Linux terminal, run the following commands:

```
sudo apt update
sudo apt install -y gtk4 libadwaita-1-0 git curl build-essential
```

These install GTK4 and other needed tools.

### Step 3: Download wayframe

Visit the wayframe page by clicking the badge above or go here:

[https://github.com/Detrayy/wayframe](https://github.com/Detrayy/wayframe)

On this page, look for the **Releases** section. Download the latest Linux-compatible package or source code.

If you download the source code, continue with the next step to build it.

### Step 4: Build wayframe from Source (Optional)

If you downloaded the source, build wayframe by doing the following:

1. Make sure Rust is installed:
   ```
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```
2. Clone the wayframe repository:
   ```
   git clone https://github.com/Detrayy/wayframe.git
   cd wayframe
   ```
3. Build the app:
   ```
   cargo build --release
   ```
4. The compiled app will be in `target/release/wayframe`.

### Step 5: Run wayframe

Once installed or built, run wayframe inside your Linux terminal by typing:

```
./wayframe <app-command>
```

Replace `<app-command>` with the program you want to open with SSD support. For example:

```
./wayframe gedit
```

This launches the gedit text editor wrapped inside a GTK4 window with server-side decorations.

## ⚙️ How wayframe Works

wayframe creates a GTK4 host window that loads your app inside it. It forces the window decorations to be handled by the GNOME compositor instead of the app. This fixes issues where apps don’t show proper borders or controls on GNOME.

This process happens automatically after you run the app with wayframe. There is no need for extra setup once it’s installed.

## 🔍 Troubleshooting

If the app does not start or windows don’t show decorations correctly, check the following:

- Make sure you run wayframe inside a Wayland session in your Linux environment.
- Verify all GTK4 libraries and dependencies are installed.
- Check if your Linux environment supports Wayland (check env variable: `echo $XDG_SESSION_TYPE` should print `wayland`).
- Confirm the app you want to run supports Wayland or XDG decorations.

You can also open the GitHub issues page for wayframe for updates or help.

## 🔄 Updating wayframe

To update wayframe, either:

- Download the latest release from the GitHub page again, or
- Pull the latest source and rebuild:

```
cd wayframe
git pull
cargo build --release
```

Repeat the installation steps after updating.

## 🗂️ Files and Structure

If you built from source, note the main files:

- `README.md` - This file
- `src/` - Source code files
- `Cargo.toml` - Rust configuration file
- `target/release/wayframe` - Compiled app executable

## 🗃️ Supported Apps

wayframe works best with Linux apps using GTK4 or libadwaita and running inside a Wayland environment. Examples include:

- GNOME apps like gedit, Nautilus, and others
- Rust-based Wayland apps using smithay libraries
- Apps lacking native SSD support but running on Wayland

## 🔗 Useful Links

- wayframe GitHub: [https://github.com/Detrayy/wayframe](https://github.com/Detrayy/wayframe)
- GTK4 Documentation: https://docs.gtk.org/gtk4/
- Rust Programming Language: https://www.rust-lang.org/
- WSL Installation Guide: https://docs.microsoft.com/en-us/windows/wsl/install

## 🧩 Contact and Support

For bugs and questions, open an issue on the GitHub page. The project maintainers monitor that regularly.

---

# Download wayframe now

[![Download wayframe](https://img.shields.io/badge/Download-wayframe-brightgreen?style=for-the-badge)](https://github.com/Detrayy/wayframe)