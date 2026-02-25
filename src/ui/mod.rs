use crate::config::app_identity::{launch_seed, metadata_title_icon};
use crate::types::{DmabufFramePayload, FramePayload, GtkToServerMsg, ServerToGtkMsg};
use crossbeam_channel::{Receiver, Sender};
use gtk4::gdk::prelude::*;
use gtk4::{gdk, glib, prelude::*};
use libadwaita::prelude::*;
use std::cell::Cell;
use std::os::fd::IntoRawFd;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn map_pointer_to_frame(
    x: f64,
    y: f64,
    widget_w: i32,
    widget_h: i32,
    frame_w: i32,
    frame_h: i32,
) -> (f64, f64) {
    let ww = widget_w.max(1) as f64;
    let wh = widget_h.max(1) as f64;
    let fw = frame_w.max(1) as f64;
    let fh = frame_h.max(1) as f64;
    let widget_ratio = ww / wh;
    let frame_ratio = fw / fh;

    let (draw_w, draw_h, off_x, off_y) = if frame_ratio > widget_ratio {
        let draw_w = ww;
        let draw_h = ww / frame_ratio;
        (draw_w, draw_h, 0.0, (wh - draw_h) * 0.5)
    } else {
        let draw_h = wh;
        let draw_w = wh * frame_ratio;
        (draw_w, draw_h, (ww - draw_w) * 0.5, 0.0)
    };

    let nx = ((x - off_x) / draw_w).clamp(0.0, 1.0);
    let ny = ((y - off_y) / draw_h).clamp(0.0, 1.0);
    (nx * (fw - 1.0), ny * (fh - 1.0))
}

fn build_dmabuf_texture(mut frame: DmabufFramePayload) -> Option<gdk::Texture> {
    let display = gdk::Display::default()?;
    let builder = gdk::DmabufTextureBuilder::new();
    builder.set_display(&display);
    builder.set_width(frame.width);
    builder.set_height(frame.height);
    builder.set_fourcc(frame.fourcc);
    builder.set_modifier(frame.modifier);
    builder.set_premultiplied(frame.premultiplied);
    builder.set_n_planes(frame.planes.len() as u32);

    for (idx, plane) in frame.planes.drain(..).enumerate() {
        builder.set_offset(idx as u32, plane.offset);
        builder.set_stride(idx as u32, plane.stride);
        builder.set_fd(idx as u32, plane.fd.into_raw_fd());
    }

    unsafe { builder.build().ok() }
}

pub fn run_ui(
    gtk_tx: Sender<GtkToServerMsg>,
    gtk_rx: Receiver<ServerToGtkMsg>,
    launch_command: String,
) {
    let (seed_title, seed_icon_name, launch_basename) = launch_seed(&launch_command);
    glib::set_prgname(Some(&launch_basename));
    glib::set_application_name(&launch_basename);

    let app = libadwaita::Application::builder().build();

    app.connect_activate(move |app| {
        let picture = gtk4::Picture::new();
        picture.set_hexpand(true);
        picture.set_vexpand(true);
        picture.set_focusable(true);
        picture.set_can_shrink(true);
        picture.set_keep_aspect_ratio(true);

        let frame_w = Rc::new(Cell::new(0_i32));
        let frame_h = Rc::new(Cell::new(0_i32));

        let motion = gtk4::EventControllerMotion::new();
        let tx_motion = gtk_tx.clone();
        let picture_for_motion = picture.clone();
        let frame_w_for_motion = frame_w.clone();
        let frame_h_for_motion = frame_h.clone();
        motion.connect_motion(move |_, x, y| {
            let (mapped_x, mapped_y) = map_pointer_to_frame(
                x,
                y,
                picture_for_motion.width(),
                picture_for_motion.height(),
                frame_w_for_motion.get(),
                frame_h_for_motion.get(),
            );
            let _ = tx_motion.send(GtkToServerMsg::PointerMotion(mapped_x, mapped_y));
        });
        picture.add_controller(motion);

        let click = gtk4::GestureClick::new();
        click.set_button(0);
        let tx_press = gtk_tx.clone();
        click.connect_pressed(move |gesture, _n_press, _x, _y| {
            let _ = tx_press.send(GtkToServerMsg::PointerButton {
                button: gesture.current_button(),
                pressed: true,
            });
        });
        let tx_release = gtk_tx.clone();
        click.connect_released(move |gesture, _n_press, _x, _y| {
            let _ = tx_release.send(GtkToServerMsg::PointerButton {
                button: gesture.current_button(),
                pressed: false,
            });
        });
        picture.add_controller(click);

        let scroll = gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::BOTH_AXES);
        let tx_scroll = gtk_tx.clone();
        scroll.connect_scroll(move |_, dx, dy| {
            let _ = tx_scroll.send(GtkToServerMsg::PointerScroll { dx, dy });
            glib::Propagation::Stop
        });

        let window = libadwaita::ApplicationWindow::builder()
            .application(app)
            .title(&seed_title)
            .default_width(800)
            .default_height(600)
            .build();
        window.set_decorated(false);
        window.set_icon_name(seed_icon_name.as_deref());

        let header = libadwaita::HeaderBar::new();
        header.set_show_end_title_buttons(true);
        header.set_show_start_title_buttons(true);
        let header_title = gtk4::Label::new(Some(&seed_title));
        header.set_title_widget(Some(&header_title));
        header.set_visible(false);

        let layout = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        layout.append(&header);
        layout.append(&picture);
        window.set_content(Some(&layout));
        window.set_focusable(true);
        picture.add_controller(scroll);

        let key_controller = gtk4::EventControllerKey::new();
        let tx_key_pressed = gtk_tx.clone();
        key_controller.connect_key_pressed(move |_, _keyval, keycode, _state| {
            let _ = tx_key_pressed.send(GtkToServerMsg::Key {
                keycode,
                pressed: true,
            });
            glib::Propagation::Proceed
        });
        let tx_key_released = gtk_tx.clone();
        key_controller.connect_key_released(move |_, _keyval, keycode, _state| {
            let _ = tx_key_released.send(GtkToServerMsg::Key {
                keycode,
                pressed: false,
            });
        });
        window.add_controller(key_controller);

        let _ = gtk_tx.send(GtkToServerMsg::Scale(picture.scale_factor()));

        let picture_for_updates = picture.clone();
        let gtk_rx_for_updates = gtk_rx.clone();
        let frame_w_for_updates = frame_w.clone();
        let frame_h_for_updates = frame_h.clone();
        let header_for_updates = header.clone();
        let header_title_for_updates = header_title.clone();
        let window_for_updates = window.clone();
        let picture_for_present = picture.clone();
        let present_started_at = Instant::now();
        let presented = Rc::new(Cell::new(false));
        let presented_for_updates = presented.clone();
        glib::timeout_add_local(Duration::from_millis(16), move || {
            let mut latest_frame: Option<FramePayload> = None;
            let mut latest_dmabuf: Option<DmabufFramePayload> = None;
            let mut latest_header_visibility: Option<bool> = None;
            let mut latest_metadata: Option<(Option<String>, Option<String>)> = None;
            while let Ok(msg) = gtk_rx_for_updates.try_recv() {
                match msg {
                    ServerToGtkMsg::NewFrame(frame) => {
                        latest_frame = Some(frame);
                    }
                    ServerToGtkMsg::NewDmabuf(frame) => {
                        latest_dmabuf = Some(frame);
                        latest_frame = None;
                    }
                    ServerToGtkMsg::SetWrapperHeader(visible) => {
                        latest_header_visibility = Some(visible);
                    }
                    ServerToGtkMsg::SetToplevelMetadata { title, app_id } => {
                        latest_metadata = Some((title, app_id));
                    }
                    ServerToGtkMsg::SetContentConstraints { min_w, min_h } => {
                        picture_for_updates.set_size_request(min_w, min_h);
                    }
                    ServerToGtkMsg::BeginWindowMove { x, y } => {
                        if let Some(surface) = window_for_updates.surface() {
                            if let Ok(toplevel) = surface.dynamic_cast::<gdk::Toplevel>() {
                                if let Some(display) = gdk::Display::default() {
                                    if let Some(seat) = display.default_seat() {
                                        if let Some(device) = seat.pointer() {
                                            toplevel.begin_move(
                                                &device,
                                                1,
                                                x,
                                                y,
                                                gdk::CURRENT_TIME,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ServerToGtkMsg::SetHostMaximized(maximized) => {
                        if maximized {
                            window_for_updates.maximize();
                        } else {
                            window_for_updates.unmaximize();
                        }
                    }
                    ServerToGtkMsg::SetHostFullscreen(fullscreen) => {
                        if fullscreen {
                            window_for_updates.fullscreen();
                        } else {
                            window_for_updates.unfullscreen();
                        }
                    }
                    ServerToGtkMsg::HostMinimize => {
                        window_for_updates.minimize();
                    }
                    ServerToGtkMsg::HostClose => {
                        window_for_updates.close();
                    }
                }
            }

            if let Some(visible) = latest_header_visibility {
                header_for_updates.set_visible(visible);
                window_for_updates.set_decorated(visible);
            }
            let saw_metadata = latest_metadata.is_some();
            if let Some((title, app_id)) = latest_metadata {
                let (effective_title, icon_name) = metadata_title_icon(title, app_id);
                window_for_updates.set_title(Some(&effective_title));
                header_title_for_updates.set_text(&effective_title);
                window_for_updates.set_icon_name(icon_name.as_deref());
            }
            if !presented_for_updates.get()
                && (saw_metadata || present_started_at.elapsed() >= Duration::from_millis(150))
            {
                window_for_updates.present();
                let _ = picture_for_present.grab_focus();
                presented_for_updates.set(true);
            }

            if let Some(frame) = latest_frame {
                let mut data = frame.data;
                if !frame.has_alpha {
                    for px in data.chunks_exact_mut(4) {
                        px[3] = 0xff;
                    }
                }

                let bytes = glib::Bytes::from_owned(data);
                let texture = gdk::MemoryTexture::new(
                    frame.width,
                    frame.height,
                    gdk::MemoryFormat::B8g8r8a8,
                    &bytes,
                    frame.stride,
                );
                frame_w_for_updates.set(frame.width);
                frame_h_for_updates.set(frame.height);
                picture_for_updates.set_paintable(Some(&texture));
            }

            if let Some(frame) = latest_dmabuf {
                if let Some(texture) = build_dmabuf_texture(frame) {
                    frame_w_for_updates.set(texture.width());
                    frame_h_for_updates.set(texture.height());
                    picture_for_updates.set_paintable(Some(&texture));
                }
            }

            glib::ControlFlow::Continue
        });

        let tx_size_scale = gtk_tx.clone();
        let picture_for_size = picture.clone();
        let last_w = Cell::new(0);
        let last_h = Cell::new(0);
        let last_scale = Cell::new(0);
        glib::timeout_add_local(Duration::from_millis(16), move || {
            let w = picture_for_size.width().max(1);
            let h = picture_for_size.height().max(1);
            let scale = picture_for_size.scale_factor().max(1);

            if w != last_w.get() || h != last_h.get() {
                last_w.set(w);
                last_h.set(h);
                let _ = tx_size_scale.send(GtkToServerMsg::Resize(w as u32, h as u32));
            }
            if scale != last_scale.get() {
                last_scale.set(scale);
                let _ = tx_size_scale.send(GtkToServerMsg::Scale(scale));
            }
            glib::ControlFlow::Continue
        });
    });

    app.run_with_args(&[""]);
}
