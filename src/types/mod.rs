use std::os::fd::OwnedFd;

#[derive(Debug, Clone)]
pub struct FramePayload {
    pub width: i32,
    pub height: i32,
    pub stride: usize,
    pub has_alpha: bool,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct DmabufPlanePayload {
    pub fd: OwnedFd,
    pub offset: u32,
    pub stride: u32,
}

#[derive(Debug)]
pub struct DmabufFramePayload {
    pub width: u32,
    pub height: u32,
    pub fourcc: u32,
    pub modifier: u64,
    pub premultiplied: bool,
    pub planes: Vec<DmabufPlanePayload>,
}

pub enum ServerToGtkMsg {
    NewFrame(FramePayload),
    NewDmabuf(DmabufFramePayload),
    SetToplevelMetadata {
        title: Option<String>,
        app_id: Option<String>,
    },
    SetWrapperHeader(bool),
    SetContentConstraints {
        min_w: i32,
        min_h: i32,
    },
    BeginWindowMove {
        x: f64,
        y: f64,
    },
    SetHostMaximized(bool),
    SetHostFullscreen(bool),
    HostMinimize,
    HostClose,
}

pub enum GtkToServerMsg {
    Resize(u32, u32),
    Scale(i32),
    PointerMotion(f64, f64),
    PointerButton { button: u32, pressed: bool },
    PointerScroll { dx: f64, dy: f64 },
    Key { keycode: u32, pressed: bool },
}
