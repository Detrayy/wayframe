use crate::config::keyboard::KeyboardConfig;
use crate::types::{
    DmabufFramePayload, DmabufPlanePayload, FramePayload, GtkToServerMsg, ServerToGtkMsg,
};
use crossbeam_channel::{Receiver, Sender};
use smithay::{
    backend::allocator::{Buffer as _, Format as DrmFormat, Fourcc, Modifier},
    backend::input::{Axis, AxisSource, ButtonState, KeyState},
    delegate_compositor, delegate_dmabuf, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_decoration, delegate_xdg_shell,
    input::{
        Seat, SeatHandler, SeatState,
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    output::{Mode as OutputMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        wayland_protocols::xdg::{
            decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode,
            shell::server::xdg_toplevel,
        },
        wayland_server::{
            Client, Display, ListeningSocket,
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::{
                wl_buffer, wl_output::WlOutput, wl_seat::WlSeat, wl_shm, wl_surface::WlSurface,
            },
        },
    },
    utils::{Logical, Point, SERIAL_COUNTER, Serial, Transform},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            BufferAssignment, CompositorClientState, CompositorHandler, CompositorState,
            SurfaceAttributes, TraversalAction, with_states, with_surface_tree_downward,
        },
        dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier, get_dmabuf},
        output::{OutputHandler, OutputManagerState},
        shell::xdg::{
            PopupSurface, PositionerState, SurfaceCachedState, ToplevelSurface, XdgShellHandler,
            XdgShellState, XdgToplevelSurfaceData,
            decoration::{XdgDecorationHandler, XdgDecorationState},
        },
        shm::{ShmHandler, ShmState, with_buffer_contents},
    },
};
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

struct WayFrameState {
    compositor_state: CompositorState,
    shm_state: ShmState,
    dmabuf_state: DmabufState,
    xdg_shell_state: XdgShellState,
    _xdg_decoration_state: XdgDecorationState,
    _dmabuf_global: DmabufGlobal,
    seat_state: SeatState<WayFrameState>,
    seat: Seat<WayFrameState>,
    output: Output,
    server_tx: Sender<ServerToGtkMsg>,
    pointer_location: Point<f64, Logical>,
    keyboard_focus: Option<WlSurface>,
    primary_surface: Option<WlSurface>,
    min_size: (i32, i32),
    max_size: (i32, i32),
    last_sent_constraints: Option<(i32, i32, i32, i32)>,
    last_frame_capture_at: Instant,
    logged_dmabuf_path: bool,
    logged_shm_path: bool,
}

#[derive(Debug, Default)]
struct WayFrameClientData {
    compositor_state: CompositorClientState,
}

impl ClientData for WayFrameClientData {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

impl CompositorHandler for WayFrameState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<WayFrameClientData>()
            .expect("WayFrameClientData missing")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        let should_capture = self
            .primary_surface
            .as_ref()
            .map(|s| s == surface)
            .unwrap_or(false);
        if should_capture {
            if self.last_frame_capture_at.elapsed() < Duration::from_millis(16) {
                release_committed_buffer(surface);
                return;
            }

            if let Some((min_w, min_h, max_w, max_h)) = extract_surface_constraints(surface) {
                let constraints = (min_w, min_h, max_w, max_h);
                if self.last_sent_constraints != Some(constraints) {
                    self.last_sent_constraints = Some(constraints);
                    self.min_size = (min_w, min_h);
                    self.max_size = (max_w, max_h);
                    let _ = self
                        .server_tx
                        .send(ServerToGtkMsg::SetContentConstraints { min_w, min_h });
                }
            }
            if let Some(frame) = extract_dmabuf_frame(surface) {
                if !self.logged_dmabuf_path {
                    self.logged_dmabuf_path = true;
                    tracing::info!("Using dmabuf frame path");
                }
                self.last_frame_capture_at = Instant::now();
                let _ = self.server_tx.send(ServerToGtkMsg::NewDmabuf(frame));
            } else if let Some(frame) = extract_shm_frame(surface) {
                if !self.logged_shm_path {
                    self.logged_shm_path = true;
                    tracing::info!("Using shm frame path");
                }
                self.last_frame_capture_at = Instant::now();
                let _ = self.server_tx.send(ServerToGtkMsg::NewFrame(frame));
            }
        }
    }
}

impl ShmHandler for WayFrameState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl BufferHandler for WayFrameState {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl DmabufHandler for WayFrameState {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        _global: &DmabufGlobal,
        _dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
        notifier: ImportNotifier,
    ) {
        if let Err(err) = notifier.successful::<WayFrameState>() {
            tracing::warn!("dmabuf import finalize failed: {}", err);
        }
    }
}

impl XdgShellHandler for WayFrameState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        tracing::info!("App created a toplevel, sending 800x600 configure");
        let _ = self.server_tx.send(ServerToGtkMsg::SetWrapperHeader(true));
        surface.with_pending_state(|state| {
            state.size = Some((800, 600).into());
            state.states.set(xdg_toplevel::State::Activated);
        });
        surface.send_configure();

        let wl_surface = surface.wl_surface().clone();
        self.keyboard_focus = Some(wl_surface.clone());
        if self.primary_surface.is_none() {
            self.primary_surface = Some(wl_surface.clone());
        }
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, Some(wl_surface), SERIAL_COUNTER.next_serial());
        }
        send_toplevel_metadata(&self.server_tx, &surface);
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {}

    fn move_request(&mut self, _surface: ToplevelSurface, _seat: WlSeat, _serial: Serial) {
        let _ = self.server_tx.send(ServerToGtkMsg::BeginWindowMove {
            x: self.pointer_location.x,
            y: self.pointer_location.y,
        });
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        let _ = self.server_tx.send(ServerToGtkMsg::SetHostMaximized(true));
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Maximized);
        });
        surface.send_configure();
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        let _ = self.server_tx.send(ServerToGtkMsg::SetHostMaximized(false));
        surface.with_pending_state(|state| {
            state.states.unset(xdg_toplevel::State::Maximized);
        });
        surface.send_configure();
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        let _ = self.server_tx.send(ServerToGtkMsg::SetHostFullscreen(true));
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Fullscreen);
        });
        surface.send_configure();
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        let _ = self
            .server_tx
            .send(ServerToGtkMsg::SetHostFullscreen(false));
        surface.with_pending_state(|state| {
            state.states.unset(xdg_toplevel::State::Fullscreen);
        });
        surface.send_configure();
    }

    fn minimize_request(&mut self, _surface: ToplevelSurface) {
        let _ = self.server_tx.send(ServerToGtkMsg::HostMinimize);
    }

    fn toplevel_destroyed(&mut self, _surface: ToplevelSurface) {
        let _ = self.server_tx.send(ServerToGtkMsg::HostClose);
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        send_toplevel_metadata(&self.server_tx, &surface);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        send_toplevel_metadata(&self.server_tx, &surface);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }
}

impl XdgDecorationHandler for WayFrameState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        tracing::info!("Client asked for decoration, forcing server-side mode");
        let _ = self.server_tx.send(ServerToGtkMsg::SetWrapperHeader(true));
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: DecorationMode) {
        let _ = self.server_tx.send(ServerToGtkMsg::SetWrapperHeader(true));
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        toplevel.send_configure();
    }

    fn unset_mode(&mut self, _toplevel: ToplevelSurface) {
        let _ = self.server_tx.send(ServerToGtkMsg::SetWrapperHeader(true));
    }
}

impl SeatHandler for WayFrameState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
}

impl OutputHandler for WayFrameState {}

delegate_compositor!(WayFrameState);
delegate_shm!(WayFrameState);
delegate_dmabuf!(WayFrameState);
delegate_xdg_shell!(WayFrameState);
delegate_xdg_decoration!(WayFrameState);
delegate_seat!(WayFrameState);
delegate_output!(WayFrameState);

fn dup_fd(fd: BorrowedFd<'_>) -> Option<OwnedFd> {
    let raw = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if raw < 0 {
        return None;
    }
    Some(unsafe { OwnedFd::from_raw_fd(raw) })
}

fn extract_dmabuf_frame(surface: &WlSurface) -> Option<DmabufFramePayload> {
    let mut captured = None;

    with_states(surface, |states| {
        let mut attrs = states.cached_state.get::<SurfaceAttributes>();
        let attrs = attrs.current();

        if let Some(BufferAssignment::NewBuffer(buffer)) = attrs.buffer.as_ref() {
            if let Ok(dmabuf) = get_dmabuf(buffer) {
                let offsets = dmabuf.offsets().collect::<Vec<_>>();
                let strides = dmabuf.strides().collect::<Vec<_>>();
                let mut planes = Vec::with_capacity(dmabuf.num_planes());

                for (idx, handle) in dmabuf.handles().enumerate() {
                    let Some(fd) = dup_fd(handle) else {
                        planes.clear();
                        break;
                    };
                    planes.push(DmabufPlanePayload {
                        fd,
                        offset: *offsets.get(idx).unwrap_or(&0),
                        stride: *strides.get(idx).unwrap_or(&0),
                    });
                }

                if planes.len() == dmabuf.num_planes() {
                    captured = Some(DmabufFramePayload {
                        width: dmabuf.width(),
                        height: dmabuf.height(),
                        fourcc: dmabuf.format().code as u32,
                        modifier: u64::from(dmabuf.format().modifier),
                        premultiplied: true,
                        planes,
                    });
                }
            }
            buffer.release();
        }
    });

    captured
}

fn extract_shm_frame(surface: &WlSurface) -> Option<FramePayload> {
    let mut captured = None;

    with_states(surface, |states| {
        let mut attrs = states.cached_state.get::<SurfaceAttributes>();
        let attrs = attrs.current();

        if let Some(BufferAssignment::NewBuffer(buffer)) = attrs.buffer.as_ref() {
            let _ = with_buffer_contents(buffer, |ptr, pool_len, data| {
                let offset = data.offset.max(0) as usize;
                let stride = data.stride.max(0) as usize;
                let height = data.height.max(0) as usize;
                let byte_len = stride.saturating_mul(height);

                let format_supported = matches!(
                    data.format,
                    wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888
                );
                if !format_supported || byte_len == 0 || offset + byte_len > pool_len {
                    return;
                }

                let bytes =
                    unsafe { std::slice::from_raw_parts(ptr.add(offset), byte_len).to_vec() };
                captured = Some(FramePayload {
                    width: data.width,
                    height: data.height,
                    stride,
                    has_alpha: data.format == wl_shm::Format::Argb8888,
                    data: bytes,
                });
            });

            buffer.release();
        }
    });

    captured
}

fn release_committed_buffer(surface: &WlSurface) {
    with_states(surface, |states| {
        let mut attrs = states.cached_state.get::<SurfaceAttributes>();
        let attrs = attrs.current();
        if let Some(BufferAssignment::NewBuffer(buffer)) = attrs.buffer.as_ref() {
            buffer.release();
        }
    });
}

fn extract_surface_constraints(surface: &WlSurface) -> Option<(i32, i32, i32, i32)> {
    let mut out = None;
    with_states(surface, |states| {
        let mut cached = states.cached_state.get::<SurfaceCachedState>();
        let current = cached.current();
        out = Some((
            current.min_size.w.max(0),
            current.min_size.h.max(0),
            current.max_size.w.max(0),
            current.max_size.h.max(0),
        ));
    });
    out
}

fn send_frames_surface_tree(surface: &WlSurface, time: u32) {
    with_surface_tree_downward(
        surface,
        (),
        |_, _, &()| TraversalAction::DoChildren(()),
        |_surf, states, &()| {
            for callback in states
                .cached_state
                .get::<SurfaceAttributes>()
                .current()
                .frame_callbacks
                .drain(..)
            {
                callback.done(time);
            }
        },
        |_, _, &()| true,
    );
}

fn gtk_button_to_linux(button: u32) -> u32 {
    match button {
        1 => 0x110,
        2 => 0x112,
        3 => 0x111,
        _ => 0x110,
    }
}

fn send_toplevel_metadata(server_tx: &Sender<ServerToGtkMsg>, surface: &ToplevelSurface) {
    let (title, app_id) = with_states(surface.wl_surface(), |states| {
        let attrs = states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .expect("XdgToplevelSurfaceData missing")
            .lock()
            .expect("XdgToplevelSurfaceData poisoned");
        (attrs.title.clone(), attrs.app_id.clone())
    });
    let _ = server_tx.send(ServerToGtkMsg::SetToplevelMetadata { title, app_id });
}

pub fn run_server(
    socket: ListeningSocket,
    socket_name: String,
    server_rx: Receiver<GtkToServerMsg>,
    server_tx: Sender<ServerToGtkMsg>,
    keyboard_config: KeyboardConfig,
) {
    tracing::info!("Started WayFrame server on socket: {}", socket_name);

    let mut display: Display<WayFrameState> = Display::new().expect("Failed to create Display");
    let display_handle = display.handle();

    let compositor_state = CompositorState::new::<WayFrameState>(&display_handle);
    let shm_state = ShmState::new::<WayFrameState>(&display_handle, vec![]);
    let mut dmabuf_state = DmabufState::new();
    let dmabuf_formats = vec![
        DrmFormat {
            code: Fourcc::Argb8888,
            modifier: Modifier::Linear,
        },
        DrmFormat {
            code: Fourcc::Xrgb8888,
            modifier: Modifier::Linear,
        },
        DrmFormat {
            code: Fourcc::Argb8888,
            modifier: Modifier::Invalid,
        },
        DrmFormat {
            code: Fourcc::Xrgb8888,
            modifier: Modifier::Invalid,
        },
    ];
    let dmabuf_global =
        dmabuf_state.create_global::<WayFrameState>(&display_handle, dmabuf_formats);
    let xdg_shell_state = XdgShellState::new::<WayFrameState>(&display_handle);
    let xdg_decoration_state = XdgDecorationState::new::<WayFrameState>(&display_handle);
    let _output_manager_state =
        OutputManagerState::new_with_xdg_output::<WayFrameState>(&display_handle);

    let mut seat_state = SeatState::new();
    let mut seat = seat_state.new_wl_seat(&display_handle, "wayframe-seat");
    seat.add_keyboard(keyboard_config.to_xkb_config(), 200, 25)
        .expect("Failed to create keyboard");
    seat.add_pointer();

    let output = Output::new(
        "wayframe-monitor".into(),
        PhysicalProperties {
            size: (1920, 1080).into(),
            subpixel: Subpixel::Unknown,
            make: "WayFrame".into(),
            model: "Virtual Display".into(),
        },
    );
    let _output_global = output.create_global::<WayFrameState>(&display_handle);
    let mode = OutputMode {
        size: (800, 600).into(),
        refresh: 60_000,
    };
    output.change_current_state(
        Some(mode),
        Some(Transform::Normal),
        Some(Scale::Integer(1)),
        Some((0, 0).into()),
    );
    output.set_preferred(mode);

    let mut state = WayFrameState {
        compositor_state,
        shm_state,
        dmabuf_state,
        xdg_shell_state,
        _xdg_decoration_state: xdg_decoration_state,
        _dmabuf_global: dmabuf_global,
        seat_state,
        seat,
        output,
        server_tx,
        pointer_location: (0.0, 0.0).into(),
        keyboard_focus: None,
        primary_surface: None,
        min_size: (0, 0),
        max_size: (0, 0),
        last_sent_constraints: None,
        last_frame_capture_at: Instant::now()
            .checked_sub(Duration::from_millis(16))
            .unwrap_or_else(Instant::now),
        logged_dmabuf_path: false,
        logged_shm_path: false,
    };

    let start_time = Instant::now();
    let mut last_frame_callbacks_at = start_time
        .checked_sub(Duration::from_millis(16))
        .unwrap_or(start_time);
    let mut queued_resize: Option<(i32, i32)> = None;
    let mut queued_scale: Option<i32> = None;
    let mut last_resize_event_at: Option<Instant> = None;
    const RESIZE_IDLE_DEBOUNCE: Duration = Duration::from_millis(45);

    loop {
        match socket.accept() {
            Ok(Some(stream)) => {
                tracing::info!("A Wayland client connected");
                let client_data = Arc::new(WayFrameClientData::default());
                if let Err(err) = display.handle().insert_client(stream, client_data) {
                    tracing::warn!("Failed to insert client: {}", err);
                }
            }
            Ok(None) => {}
            Err(err) => tracing::warn!("Socket accept error: {}", err),
        }

        while let Ok(msg) = server_rx.try_recv() {
            let now = start_time.elapsed().as_millis() as u32;
            match msg {
                GtkToServerMsg::Resize(w, h) => {
                    queued_resize = Some((w as i32, h as i32));
                    last_resize_event_at = Some(Instant::now());
                }
                GtkToServerMsg::Scale(scale) => queued_scale = Some(scale),
                GtkToServerMsg::PointerMotion(x, y) => {
                    state.pointer_location = (x, y).into();
                    if let Some(pointer) = state.seat.get_pointer() {
                        let location = state.pointer_location;
                        let focus = state
                            .keyboard_focus
                            .clone()
                            .map(|s| (s, (0.0_f64, 0.0_f64).into()));
                        pointer.motion(
                            &mut state,
                            focus,
                            &MotionEvent {
                                location,
                                serial: SERIAL_COUNTER.next_serial(),
                                time: now,
                            },
                        );
                    }
                }
                GtkToServerMsg::PointerButton { button, pressed } => {
                    if let Some(pointer) = state.seat.get_pointer() {
                        pointer.button(
                            &mut state,
                            &ButtonEvent {
                                serial: SERIAL_COUNTER.next_serial(),
                                time: now,
                                button: gtk_button_to_linux(button),
                                state: if pressed {
                                    ButtonState::Pressed
                                } else {
                                    ButtonState::Released
                                },
                            },
                        );
                        pointer.frame(&mut state);
                    }
                }
                GtkToServerMsg::PointerScroll { dx, dy } => {
                    if let Some(pointer) = state.seat.get_pointer() {
                        const SCROLL_SCALE: f64 = 120.0;
                        let mut axis = AxisFrame::new(now).source(AxisSource::Wheel);
                        if dx != 0.0 {
                            axis = axis
                                .value(Axis::Horizontal, dx * SCROLL_SCALE)
                                .v120(Axis::Horizontal, (dx * 120.0).round() as i32);
                        }
                        if dy != 0.0 {
                            axis = axis
                                .value(Axis::Vertical, dy * SCROLL_SCALE)
                                .v120(Axis::Vertical, (dy * 120.0).round() as i32);
                        }
                        pointer.axis(&mut state, axis);
                        pointer.frame(&mut state);
                    }
                }
                GtkToServerMsg::Key { keycode, pressed } => {
                    if let Some(keyboard) = state.seat.get_keyboard() {
                        if let Some(focus) = state.keyboard_focus.clone() {
                            keyboard.set_focus(
                                &mut state,
                                Some(focus),
                                SERIAL_COUNTER.next_serial(),
                            );
                        }
                        let evdev_keycode = keycode.saturating_sub(8);
                        let _ = keyboard.input::<(), _>(
                            &mut state,
                            evdev_keycode.into(),
                            if pressed {
                                KeyState::Pressed
                            } else {
                                KeyState::Released
                            },
                            SERIAL_COUNTER.next_serial(),
                            now,
                            |_, _, _| FilterResult::Forward,
                        );
                    }
                }
            }
        }

        if let Some((mut w, mut h)) = queued_resize {
            if state.min_size.0 > 0 {
                w = w.max(state.min_size.0);
            }
            if state.min_size.1 > 0 {
                h = h.max(state.min_size.1);
            }
            if state.max_size.0 > 0 {
                w = w.min(state.max_size.0);
            }
            if state.max_size.1 > 0 {
                h = h.min(state.max_size.1);
            }

            let idle_long_enough = last_resize_event_at
                .map(|t| Instant::now().duration_since(t) >= RESIZE_IDLE_DEBOUNCE)
                .unwrap_or(true);

            if idle_long_enough {
                queued_resize = None;
                state.output.change_current_state(
                    Some(OutputMode {
                        size: (w, h).into(),
                        refresh: 60_000,
                    }),
                    None,
                    None,
                    None,
                );
                for toplevel in state.xdg_shell_state.toplevel_surfaces() {
                    toplevel.with_pending_state(|st| {
                        st.size = Some((w, h).into());
                    });
                    toplevel.send_configure();
                }
            }
        }

        if let Some(scale) = queued_scale.take() {
            state
                .output
                .change_current_state(None, None, Some(Scale::Integer(scale.max(1))), None);
        }

        if let Err(err) = display.dispatch_clients(&mut state) {
            tracing::warn!("Error dispatching clients: {}", err);
        }

        let loop_now = Instant::now();
        let toplevels = state.xdg_shell_state.toplevel_surfaces();
        if let Some(primary) = state.primary_surface.as_ref() {
            let primary_still_alive = toplevels.iter().any(|t| t.wl_surface() == primary);
            if !primary_still_alive {
                let _ = state.server_tx.send(ServerToGtkMsg::HostClose);
                break;
            }
        }
        if loop_now.duration_since(last_frame_callbacks_at) >= Duration::from_millis(16) {
            last_frame_callbacks_at = loop_now;
            let now = start_time.elapsed().as_millis() as u32;
            for surface in toplevels {
                send_frames_surface_tree(surface.wl_surface(), now);
            }
        }

        if let Err(err) = display.flush_clients() {
            tracing::warn!("Error flushing clients: {}", err);
        }

        thread::sleep(Duration::from_millis(4));
    }
}
