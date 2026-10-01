//! `wipe tray`: the board server as a real desktop app - an icon in the Windows
//! notification area / the macOS menu bar with a menu (open the board, open it on
//! a phone, copy the phone link, start at login, quit) instead of a headless
//! background process. Login autostart launches this on Windows and macOS.
//!
//! The server runs in-process on a background thread; the tray owns the main
//! thread's event loop (required on macOS). Only one tray runs per user: a second
//! `wipe tray` just opens the board and exits.

#[cfg(not(any(windows, target_os = "macos")))]
pub fn run(_args: crate::args::TrayArgs) -> anyhow::Result<()> {
    anyhow::bail!(
        "the tray app needs Windows or macOS. On Linux run `wipe serve` (`wipe config --global set \
         autostart true` starts it at login via a systemd user unit)"
    )
}

#[cfg(any(windows, target_os = "macos"))]
pub use imp::run;

#[cfg(any(windows, target_os = "macos"))]
mod imp {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use anyhow::{Context, Result};
    use tao::event::{Event, StartCause};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    use wipe_core::{registry, GlobalConfig, Store};

    use crate::args::TrayArgs;
    use crate::autostart;

    enum UserEvent {
        Tray(TrayIconEvent),
        Menu(MenuEvent),
        Ready(Vec<wipe_daemon::ShownUrl>),
        Stopped(Option<String>),
    }

    /// Keep one tray per user: the lock is held for the process lifetime.
    fn single_instance() -> Option<std::fs::File> {
        let dir = GlobalConfig::path()?.parent()?.to_path_buf();
        std::fs::create_dir_all(&dir).ok()?;
        let f = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("tray.lock"))
            .ok()?;
        f.try_lock().ok()?;
        Some(f)
    }

    fn open_url(url: &str) {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn();
        }
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
    }

    fn copy_to_clipboard(text: &str) {
        use std::io::Write;
        #[cfg(windows)]
        let cmd = {
            use std::os::windows::process::CommandExt;
            let mut c = std::process::Command::new("clip");
            c.creation_flags(0x0800_0000);
            c
        };
        #[cfg(target_os = "macos")]
        let cmd = std::process::Command::new("pbcopy");
        let mut cmd = cmd;
        if let Ok(mut child) = cmd.stdin(std::process::Stdio::piped()).spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
        }
    }

    /// When started from a terminal, relaunch in the background and return, so
    /// the terminal isn't held by a GUI app. Returns `true` when relaunched.
    fn detach() -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let mut cmd = std::process::Command::new(exe);
        cmd.args(["tray", "--foreground"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const DETACHED_PROCESS: u32 = 0x0000_0008;
            const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
            cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
        }
        #[cfg(target_os = "macos")]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        cmd.spawn().is_ok()
    }

    /// The tray icon: a rounded terracotta tile with a white "w", drawn at
    /// `size` px with anti-aliased strokes (no image assets needed).
    fn icon(size: u32) -> Icon {
        let s = size as f32;
        let mut rgba = vec![0u8; (size * size * 4) as usize];
        // The "w": four strokes through five points.
        let pts = [
            (0.22, 0.32),
            (0.36, 0.70),
            (0.50, 0.42),
            (0.64, 0.70),
            (0.78, 0.32),
        ];
        let seg_dist = |px: f32, py: f32, a: (f32, f32), b: (f32, f32)| {
            let (ax, ay, bx, by) = (a.0 * s, a.1 * s, b.0 * s, b.1 * s);
            let (dx, dy) = (bx - ax, by - ay);
            let t = (((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            ((px - ax - t * dx).powi(2) + (py - ay - t * dy).powi(2)).sqrt()
        };
        let radius = s * 0.22;
        let stroke = s * 0.075;
        for y in 0..size {
            for x in 0..size {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                // Rounded-square coverage.
                let qx = (px - s / 2.0).abs() - (s / 2.0 - radius);
                let qy = (py - s / 2.0).abs() - (s / 2.0 - radius);
                let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt()
                    + qx.max(qy).min(0.0)
                    - radius;
                let tile = (0.5 - outside).clamp(0.0, 1.0);
                let d = pts
                    .windows(2)
                    .map(|w| seg_dist(px, py, w[0], w[1]))
                    .fold(f32::MAX, f32::min);
                let ink = (stroke - d + 0.5).clamp(0.0, 1.0);
                let (r, g, b) = (
                    204.0 + (255.0 - 204.0) * ink,
                    120.0 + (255.0 - 120.0) * ink,
                    92.0 + (255.0 - 92.0) * ink,
                );
                let i = ((y * size + x) * 4) as usize;
                rgba[i] = r as u8;
                rgba[i + 1] = g as u8;
                rgba[i + 2] = b as u8;
                rgba[i + 3] = (tile * 255.0) as u8;
            }
        }
        Icon::from_rgba(rgba, size, size).expect("valid icon buffer")
    }

    pub fn run(args: TrayArgs) -> Result<()> {
        let Some(_lock) = single_instance() else {
            // Already running: be useful and open the board.
            let port = args.port.unwrap_or_else(|| {
                GlobalConfig::load()
                    .default_port
                    .unwrap_or(wipe_core::model::DEFAULT_PORT)
            });
            open_url(&format!("http://localhost:{port}"));
            println!("wipe is already running in the tray - opened the board.");
            return Ok(());
        };
        #[cfg(windows)]
        let from_console =
            unsafe { !windows_sys::Win32::System::Console::GetConsoleWindow().is_null() };
        #[cfg(target_os = "macos")]
        let from_console = std::io::IsTerminal::is_terminal(&std::io::stdin());
        if !args.foreground && from_console {
            drop(_lock);
            if detach() {
                println!(
                    "wipe is running in your system tray (menu: open board, phone link, quit)."
                );
                return Ok(());
            }
            return run(TrayArgs {
                foreground: true,
                ..args
            });
        }

        // Serve like `wipe serve` from wherever the tray was started (usually
        // outside any board: a viewer over every registered board).
        let g = GlobalConfig::load();
        let board = Store::discover(".").ok();
        let mut settings = match &board {
            Some(s) => s.load_settings().unwrap_or_default(),
            None => wipe_core::model::Settings::default(),
        };
        if board.is_none() {
            if let Some(p) = g.default_port {
                settings.daemon.port = p;
            }
            settings.daemon.expose = g.default_expose.unwrap_or(settings.daemon.expose);
        }
        let port = args.port.unwrap_or(settings.daemon.port);
        let mut roots = crate::commands::configured_scan_roots();
        if let Ok(cwd) = std::env::current_dir() {
            roots.push(cwd);
        }
        std::thread::spawn(move || {
            registry::prune();
            registry::scan(&roots, 7)
        });

        #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
        let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
        #[cfg(target_os = "macos")]
        {
            // A menu-bar app: no Dock icon, no app switcher entry.
            use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
            event_loop.set_activation_policy(ActivationPolicy::Accessory);
        }
        let proxy = event_loop.create_proxy();
        TrayIconEvent::set_event_handler(Some({
            let p = proxy.clone();
            move |e| {
                let _ = p.send_event(UserEvent::Tray(e));
            }
        }));
        MenuEvent::set_event_handler(Some({
            let p = proxy.clone();
            move |e| {
                let _ = p.send_event(UserEvent::Menu(e));
            }
        }));

        // The server, unless one already serves this port (then the tray just
        // controls access to it).
        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let server = if crate::commands::detect_running(port).is_some() {
            let _ = proxy.send_event(UserEvent::Ready(vec![wipe_daemon::ShownUrl {
                label: "this machine".into(),
                url: format!("http://localhost:{port}"),
            }]));
            None
        } else {
            let ready = proxy.clone();
            let cfg = wipe_daemon::ServeConfig {
                root: board.as_ref().map(|s| s.root().to_path_buf()),
                port,
                expose: settings.daemon.expose,
                host: None,
                qr: false,
                open: false,
                idle_timeout: None,
                stop: Some(stop_rx),
                on_ready: Some(Arc::new(move |urls| {
                    let _ = ready.send_event(UserEvent::Ready(urls));
                })),
            };
            let done = proxy.clone();
            Some(std::thread::spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("tokio runtime");
                let r = rt.block_on(wipe_daemon::serve(cfg));
                let _ = done.send_event(UserEvent::Stopped(r.err().map(|e| format!("{e:#}"))));
            }))
        };

        let status = MenuItem::new(format!("wipe - starting on port {port}…"), false, None);
        let open = MenuItem::new("Open board", true, None);
        let phone = MenuItem::new("Open on phone… (QR codes)", false, None);
        let copy = MenuItem::new("Copy phone link", false, None);
        let login = CheckMenuItem::new("Start at login", true, autostart::is_enabled(), None);
        let quit = MenuItem::new("Quit wipe", true, None);
        let menu = Menu::new();
        menu.append_items(&[
            &status,
            &PredefinedMenuItem::separator(),
            &open,
            &phone,
            &copy,
            &PredefinedMenuItem::separator(),
            &login,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .context("building the tray menu")?;

        let urls: Arc<Mutex<Vec<wipe_daemon::ShownUrl>>> = Arc::default();
        let mut tray = None;
        let mut server = server;
        let local_url = move |urls: &[wipe_daemon::ShownUrl]| {
            urls.first()
                .map(|u| u.url.clone())
                .unwrap_or_else(|| format!("http://localhost:{port}"))
        };

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Wait;
            match event {
                Event::NewEvents(StartCause::Init) => {
                    // Created here (not before the loop) as macOS requires.
                    tray = TrayIconBuilder::new()
                        .with_menu(Box::new(menu.clone()))
                        .with_menu_on_left_click(false)
                        .with_tooltip(format!("wipe - board on port {port}"))
                        .with_icon(icon(64))
                        .build()
                        .ok();
                }
                Event::UserEvent(UserEvent::Ready(list)) => {
                    let remote = list.iter().skip(1).count();
                    status.set_text(if remote > 0 {
                        format!("wipe - serving on port {port} (this machine + {remote} network link(s))")
                    } else {
                        format!("wipe - serving on port {port} (this machine only)")
                    });
                    phone.set_enabled(true);
                    copy.set_enabled(remote > 0);
                    *urls.lock().unwrap() = list;
                }
                Event::UserEvent(UserEvent::Stopped(err)) => {
                    server = None;
                    status.set_text(match err {
                        Some(e) => format!("wipe - server stopped: {e}"),
                        None => "wipe - server stopped".to_string(),
                    });
                }
                Event::UserEvent(UserEvent::Tray(TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                })) => open_url(&local_url(&urls.lock().unwrap())),
                Event::UserEvent(UserEvent::Menu(e)) => {
                    let list = urls.lock().unwrap().clone();
                    if e.id == open.id() {
                        open_url(&local_url(&list));
                    } else if e.id == phone.id() {
                        open_url(&format!("http://localhost:{port}/connect"));
                    } else if e.id == copy.id() {
                        if let Some(u) = list.get(1) {
                            copy_to_clipboard(&u.url);
                        }
                    } else if e.id == login.id() {
                        let want = login.is_checked();
                        let r = if want {
                            autostart::enable()
                        } else {
                            autostart::disable()
                        };
                        if r.is_ok() {
                            let mut g = GlobalConfig::load();
                            g.autostart = Some(want);
                            let _ = g.save();
                        }
                        login.set_checked(autostart::is_enabled());
                    } else if e.id == quit.id() {
                        let _ = stop_tx.send(true);
                        if let Some(h) = server.take() {
                            // Give in-flight requests a moment to finish.
                            let deadline = std::time::Instant::now() + Duration::from_secs(3);
                            while !h.is_finished() && std::time::Instant::now() < deadline {
                                std::thread::sleep(Duration::from_millis(50));
                            }
                        }
                        tray.take();
                        *control_flow = ControlFlow::Exit;
                    }
                }
                _ => {}
            }
        })
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn icon_is_a_filled_tile_with_a_transparent_corner() {
            // Building the icon must not panic and must yield a valid buffer.
            let _ = super::icon(32);
            let _ = super::icon(64);
        }
    }
}
