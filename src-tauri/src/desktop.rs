//! Window chrome and the system “now playing” surface.
//!
//! Windows replaces the taskbar thumbnail with a media card and shows the song on the lock screen.
//! The optional corner bar is a second window that only sends commands back to the main one.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder, WindowEvent};

static ALLOW_CLOSE: AtomicBool = AtomicBool::new(false);
static TRAY_SHOW: Mutex<Option<MenuItem<tauri::Wry>>> = Mutex::new(None);
static TRAY_QUIT: Mutex<Option<MenuItem<tauri::Wry>>> = Mutex::new(None);

static MINI_ON: AtomicBool = AtomicBool::new(false);
static MINI_PLACED: AtomicBool = AtomicBool::new(false);
static TRACK: LazyLock<Mutex<DesktopTrack>> = LazyLock::new(|| Mutex::new(DesktopTrack::default()));

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(default)]
    pub cover_path: Option<String>,
    pub playing: bool,
    pub favorite: bool,
    pub duration: f64,
    pub position: f64,
    #[serde(default = "dark")]
    pub theme: String,
    #[serde(default = "english")]
    pub language: String,
    pub tips: Tips,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tips {
    pub previous: String,
    pub play: String,
    pub pause: String,
    pub next: String,
    pub like: String,
}

fn dark() -> String {
    "dark".into()
}
fn english() -> String {
    "en".into()
}

impl Default for Tips {
    fn default() -> Self {
        Self { previous: "Previous".into(), play: "Play".into(), pause: "Pause".into(), next: "Next".into(), like: "Like".into() }
    }
}

impl Default for DesktopTrack {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            cover_path: None,
            playing: false,
            favorite: false,
            duration: 0.0,
            position: 0.0,
            theme: dark(),
            language: english(),
            tips: Tips::default(),
        }
    }
}

pub fn install(app: &AppHandle) {
    if let Ok(mini) = WebviewWindowBuilder::new(app, "mini", WebviewUrl::App("index.html".into()))
        .title("Now playing")
        .inner_size(400.0, 76.0)
        .min_inner_size(320.0, 76.0)
        .max_inner_size(480.0, 76.0)
        .decorations(false)
        .shadow(true)
        .background_color(tauri::window::Color(27, 23, 20, 255))
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .focused(false)
        .build()
    {
        let mini_window = mini.clone();
        mini.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = mini_window.hide();
            }
        });
    }
    install_close(app);
    install_tray(app);
    #[cfg(windows)]
    windows_media::attach(app);
}

fn install_close(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            if ALLOW_CLOSE.load(Ordering::Relaxed) {
                return;
            }
            api.prevent_close();
            let _ = handle.emit("close-requested", ());
        }
    });
}

fn install_tray(app: &AppHandle) {
    let Ok(show_item) = MenuItem::with_id(app, "show", "Show", true, None::<&str>) else { return };
    let Ok(quit_item) = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>) else { return };
    let Ok(menu) = Menu::with_items(app, &[&show_item, &quit_item]) else { return };
    if let Ok(mut slot) = TRAY_SHOW.lock() {
        *slot = Some(show_item);
    }
    if let Ok(mut slot) = TRAY_QUIT.lock() {
        *slot = Some(quit_item);
    }
    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .tooltip("TunePeerly")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => quit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            let open = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if open {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    let _ = builder.build(app);
}

pub fn set_tray_tooltip(app: &AppHandle, name: &str) {
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(name));
    }
}

pub fn set_tray_labels(show: &str, quit_label: &str) {
    if let Ok(guard) = TRAY_SHOW.lock() {
        if let Some(item) = guard.as_ref() {
            let _ = item.set_text(show);
        }
    }
    if let Ok(guard) = TRAY_QUIT.lock() {
        if let Some(item) = guard.as_ref() {
            let _ = item.set_text(quit_label);
        }
    }
}

pub fn quit(app: &AppHandle) {
    ALLOW_CLOSE.store(true, Ordering::Relaxed);
    app.exit(0);
}

pub fn hide_to_tray(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("mini") {
        let _ = window.hide();
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else { return };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn update(app: &AppHandle, track: DesktopTrack) {
    if let Ok(mut slot) = TRACK.lock() {
        *slot = track.clone();
    }
    let _ = app.emit_to("mini", "desktop-state", &track);
    #[cfg(windows)]
    windows_media::publish(app, track);
}

pub fn current() -> DesktopTrack {
    TRACK.lock().map(|guard| guard.clone()).unwrap_or_default()
}

pub fn sync_mini(app: &AppHandle, enabled: bool) {
    MINI_ON.store(enabled, Ordering::Relaxed);
    let minimized = app.get_webview_window("main").and_then(|window| window.is_minimized().ok()).unwrap_or(false);
    apply_mini(app, enabled && minimized);
}

pub fn command(app: &AppHandle, action: &str) {
    if action == "restore" {
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.unminimize();
            let _ = main.show();
            let _ = main.set_focus();
        }
        apply_mini(app, false);
        return;
    }
    let _ = app.emit_to("main", "desktop-action", serde_json::json!({ "action": action }));
}

fn apply_mini(app: &AppHandle, show: bool) {
    let Some(mini) = app.get_webview_window("mini") else { return };
    if !show {
        let _ = mini.hide();
        return;
    }
    // The corner is only the first position. After that, a drag is kept.
    if !MINI_PLACED.swap(true, Ordering::AcqRel) {
        if let Some(main) = app.get_webview_window("main") {
            if let Ok(Some(monitor)) = main.current_monitor() {
                let area = monitor.work_area();
                let scale = monitor.scale_factor();
                let width = (400.0 * scale) as i32;
                let height = (76.0 * scale) as i32;
                let x = area.position.x + area.size.width as i32 - width - (16.0 * scale) as i32;
                let y = area.position.y + area.size.height as i32 - height - (16.0 * scale) as i32;
                let _ = mini.set_position(PhysicalPosition::new(x, y));
            }
        }
    }
    let _ = mini.show();
}

#[cfg(windows)]
mod windows_media {
    use std::cell::RefCell;
    use std::sync::Mutex;

    use super::{apply_mini, command, DesktopTrack, MINI_ON};
    use tauri::{AppHandle, Manager};
    use windows::{
            core::{factory, HSTRING},
        Foundation::{TimeSpan, TypedEventHandler},
        Media::{
            MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls, SystemMediaTransportControlsButton,
            SystemMediaTransportControlsButtonPressedEventArgs, SystemMediaTransportControlsTimelineProperties,
        },
        Storage::Streams::{DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference},
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, WPARAM},
            Graphics::{
                Dwm::{DwmInvalidateIconicBitmaps, DwmSetIconicThumbnail, DwmSetWindowAttribute, DWMWINDOWATTRIBUTE},
                Gdi::{
                    CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, HGDIOBJ,
                },
            },
            System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            System::WinRT::ISystemMediaTransportControlsInterop,
            UI::{
                Shell::{
                    DefSubclassProc, ITaskbarList3, SetWindowSubclass, TaskbarList, THUMBBUTTON, THUMBBUTTONMASK, THBF_ENABLED,
                    THBN_CLICKED, THB_FLAGS, THB_ICON, THB_TOOLTIP,
                },
                WindowsAndMessaging::{CreateIconIndirect, GetSystemMetrics, ICONINFO, SM_CXSMICON, WM_COMMAND, WM_SIZE},
            },
        },
    };

    const DWMWA_FORCE_ICONIC_REPRESENTATION: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(7);
    const DWMWA_HAS_ICONIC_BITMAP: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(10);
    const DWMWA_BORDER_COLOR: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(34);
    const DWMWA_COLOR_NONE: u32 = 0xFFFFFFFE;
    const DWMWA_WINDOW_CORNER_PREFERENCE: DWMWINDOWATTRIBUTE = DWMWINDOWATTRIBUTE(33);
    const DWMWCP_ROUND: u32 = 2;
    const SIZE_RESTORED: u32 = 0;
    const SIZE_MINIMIZED: u32 = 1;
    const SIZE_MAXIMIZED: u32 = 2;
    const ID_PREV: u32 = 1;
    const ID_PLAY: u32 = 2;
    const ID_NEXT: u32 = 3;
    const ID_LIKE: u32 = 4;

    struct Shell {
        hwnd: HWND,
        controls: SystemMediaTransportControls,
        taskbar: ITaskbarList3,
        buttons: bool,
        play_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        pause_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        prev_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        next_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        like_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        liked_icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        shown: String,
    }

    thread_local! {
        static SHELL: RefCell<Option<Shell>> = const { RefCell::new(None) };
    }
    static APP: Mutex<Option<AppHandle>> = Mutex::new(None);

    pub fn attach(app: &AppHandle) {
        if let Ok(mut slot) = APP.lock() {
            *slot = Some(app.clone());
        }
        let Some(window) = app.get_webview_window("main") else { return };
        let Ok(raw) = window.hwnd() else { return };
        let hwnd = HWND(raw.0);
        unsafe {
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, &DWMWA_COLOR_NONE as *const _ as *const _, 4);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND as *const _ as *const _, 4);
            let yes: i32 = 1;
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_FORCE_ICONIC_REPRESENTATION, &yes as *const _ as *const _, 4);
            let _ = DwmSetWindowAttribute(hwnd, DWMWA_HAS_ICONIC_BITMAP, &yes as *const _ as *const _, 4);
            let _ = SetWindowSubclass(hwnd, Some(subclass), 1, 0);
        }
        let Ok(interop) = factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>() else { return };
        let Ok(controls) = (unsafe { interop.GetForWindow::<SystemMediaTransportControls>(hwnd) }) else { return };
        let _ = controls.SetIsEnabled(false);
        let _ = controls.SetIsPlayEnabled(true);
        let _ = controls.SetIsPauseEnabled(true);
        let _ = controls.SetIsNextEnabled(true);
        let _ = controls.SetIsPreviousEnabled(true);
        let handler = TypedEventHandler::new(|_, args: windows::core::Ref<SystemMediaTransportControlsButtonPressedEventArgs>| {
            if let Some(args) = args.as_ref() {
                if let Ok(button) = args.Button() {
                    let action = match button {
                        SystemMediaTransportControlsButton::Play => "play",
                        SystemMediaTransportControlsButton::Pause => "pause",
                        SystemMediaTransportControlsButton::Next => "next",
                        SystemMediaTransportControlsButton::Previous => "previous",
                        _ => "",
                    };
                    if !action.is_empty() {
                        command_from_ui(action);
                    }
                }
            }
            Ok(())
        });
        let _ = controls.ButtonPressed(&handler);
        let Ok(taskbar) = (unsafe { CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER) }) else { return };
        let _ = unsafe { taskbar.HrInit() };
        SHELL.with(|slot| {
            *slot.borrow_mut() = Some(Shell {
                hwnd,
                controls,
                taskbar,
                buttons: false,
                play_icon: glyph_icon(Glyph::Play),
                pause_icon: glyph_icon(Glyph::Pause),
                prev_icon: glyph_icon(Glyph::Prev),
                next_icon: glyph_icon(Glyph::Next),
                like_icon: glyph_icon(Glyph::Heart),
                liked_icon: glyph_icon(Glyph::HeartFill),
                shown: String::new(),
            });
        });
    }

    pub fn publish(app: &AppHandle, track: DesktopTrack) {
        let app = app.clone();
        let _ = app.clone().run_on_main_thread(move || apply(&app, &track));
    }

    fn command_from_ui(action: &str) {
        if let Some(app) = APP.lock().ok().and_then(|guard| guard.clone()) {
            command(&app, action);
        }
    }

    fn apply(app: &AppHandle, track: &DesktopTrack) {
        SHELL.with(|slot| {
            let mut guard = slot.borrow_mut();
            let Some(shell) = guard.as_mut() else { return };
            let enabled = !track.title.is_empty();
            let signature = format!("{}|{}|{}|{}", track.title, track.artist, track.album, track.cover_path.as_deref().unwrap_or(""));
            let metadata_changed = signature != shell.shown;
            if metadata_changed {
                shell.shown = signature;
            }
            let _ = shell.controls.SetIsEnabled(enabled);
            let _ = shell.controls.SetPlaybackStatus(if !enabled {
                MediaPlaybackStatus::Closed
            } else if track.playing {
                MediaPlaybackStatus::Playing
            } else {
                MediaPlaybackStatus::Paused
            });
            if metadata_changed {
                if let Ok(updater) = shell.controls.DisplayUpdater() {
                    let _ = updater.SetType(MediaPlaybackType::Music);
                    if let Ok(music) = updater.MusicProperties() {
                        let _ = music.SetTitle(&HSTRING::from(&track.title));
                        let _ = music.SetArtist(&HSTRING::from(&track.artist));
                        let _ = music.SetAlbumTitle(&HSTRING::from(&track.album));
                    }
                    if let Some(path) = track.cover_path.as_deref() {
                        if let Some(reference) = stream_from_file(path) {
                            let _ = updater.SetThumbnail(&reference);
                        }
                    }
                    let _ = updater.Update();
                }
            }
            if let Ok(timeline) = SystemMediaTransportControlsTimelineProperties::new() {
                let end = seconds(track.duration);
                let position = seconds(track.position.min(track.duration));
                let _ = timeline.SetStartTime(seconds(0.0));
                let _ = timeline.SetEndTime(end);
                let _ = timeline.SetMinSeekTime(seconds(0.0));
                let _ = timeline.SetMaxSeekTime(end);
                let _ = timeline.SetPosition(position);
                let _ = shell.controls.UpdateTimelineProperties(&timeline);
            }
            refresh_buttons(shell, track);
            if metadata_changed {
                let _ = unsafe { DwmInvalidateIconicBitmaps(shell.hwnd) };
            }
            let _ = app;
        });
    }

    fn seconds(value: f64) -> TimeSpan {
        TimeSpan { Duration: (value.max(0.0) * 10_000_000.0) as i64 }
    }

    fn stream_from_file(path: &str) -> Option<RandomAccessStreamReference> {
        let bytes = std::fs::read(path).ok()?;
        let stream = InMemoryRandomAccessStream::new().ok()?;
        let writer = DataWriter::CreateDataWriter(&stream).ok()?;
        writer.WriteBytes(&bytes).ok()?;
        writer.StoreAsync().ok()?.get().ok()?;
        writer.DetachStream().ok()?;
        stream.Seek(0).ok()?;
        RandomAccessStreamReference::CreateFromStream(&stream).ok()
    }

    fn refresh_buttons(shell: &mut Shell, track: &DesktopTrack) {
        let play_tip = if track.playing { &track.tips.pause } else { &track.tips.play };
        let play_icon = if track.playing { shell.pause_icon } else { shell.play_icon };
        let like_icon = if track.favorite { shell.liked_icon } else { shell.like_icon };
        let mut buttons = [
            thumb(ID_PREV, shell.prev_icon, &track.tips.previous),
            thumb(ID_PLAY, play_icon, play_tip),
            thumb(ID_NEXT, shell.next_icon, &track.tips.next),
            thumb(ID_LIKE, like_icon, &track.tips.like),
        ];
        unsafe {
            if shell.buttons {
                let _ = shell.taskbar.ThumbBarUpdateButtons(shell.hwnd, &buttons);
            } else if shell.taskbar.ThumbBarAddButtons(shell.hwnd, &buttons).is_ok() {
                shell.buttons = true;
            }
        }
        let _ = &mut buttons;
    }

    fn thumb(id: u32, icon: windows::Win32::UI::WindowsAndMessaging::HICON, tip: &str) -> THUMBBUTTON {
        let mut button = THUMBBUTTON::default();
        button.dwMask = THUMBBUTTONMASK(THB_ICON.0 | THB_TOOLTIP.0 | THB_FLAGS.0);
        button.iId = id;
        button.hIcon = icon;
        button.dwFlags = THBF_ENABLED;
        let mut encoded = tip.encode_utf16();
        for slot in button.szTip.iter_mut().take(259) {
            *slot = encoded.next().unwrap_or(0);
            if *slot == 0 {
                break;
            }
        }
        button
    }

    unsafe extern "system" fn subclass(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
        match msg {
            WM_DWMSEND => {
                let width = ((lparam.0 as u32) >> 16) as i32;
                let height = ((lparam.0 as u32) & 0xFFFF) as i32;
                paint_thumbnail(hwnd, width.max(1), height.max(1));
                LRESULT(0)
            }
            WM_COMMAND => {
                let note = ((wparam.0 as u32) >> 16) as u16;
                let button = (wparam.0 as u32) & 0xFFFF;
                if note == THBN_CLICKED as u16 {
                    let action = match button {
                        ID_PREV => "previous",
                        ID_PLAY => "toggle",
                        ID_NEXT => "next",
                        ID_LIKE => "like",
                        _ => "",
                    };
                    if !action.is_empty() {
                        command_from_ui(action);
                        return LRESULT(0);
                    }
                }
                unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
            }
            WM_SIZE => {
                let kind = wparam.0 as u32;
                if kind == SIZE_MINIMIZED || kind == SIZE_RESTORED || kind == SIZE_MAXIMIZED {
                    if let Some(app) = APP.lock().ok().and_then(|guard| guard.clone()) {
                        apply_mini(&app, MINI_ON.load(std::sync::atomic::Ordering::Relaxed) && kind == SIZE_MINIMIZED);
                    }
                }
                unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
            }
            _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
        }
    }

    const WM_DWMSEND: u32 = 0x0323;

    fn paint_thumbnail(hwnd: HWND, width: i32, height: i32) {
        let track = super::current();
        unsafe {
            let screen = GetDC(None);
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = 0;
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let Ok(bitmap) = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0) else {
                ReleaseDC(None, screen);
                return;
            };
            if !bits.is_null() {
                fill_cover(bits as *mut u8, width, height, track.cover_path.as_deref());
            }
            let _ = DwmSetIconicThumbnail(hwnd, bitmap, 0);
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            ReleaseDC(None, screen);
        }
    }

    fn fill_cover(bits: *mut u8, width: i32, height: i32, path: Option<&str>) {
        let count = (width * height) as usize;
        let pixels = unsafe { std::slice::from_raw_parts_mut(bits, count * 4) };
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[0] = 0x14;
            pixel[1] = 0x12;
            pixel[2] = 0x10;
            pixel[3] = 0xFF;
        }
        let Some(path) = path else { return };
        let Ok(image) = image::open(path) else { return };
        let rgba = image.to_rgba8();
        let (source_w, source_h) = rgba.dimensions();
        if source_w == 0 || source_h == 0 || width <= 0 || height <= 0 {
            return;
        }
        let scale = (width as f32 / source_w as f32).max(height as f32 / source_h as f32);
        let crop_w = ((width as f32 / scale).round() as u32).clamp(1, source_w);
        let crop_h = ((height as f32 / scale).round() as u32).clamp(1, source_h);
        let origin_x = (source_w - crop_w) / 2;
        let origin_y = (source_h - crop_h) / 2;
        let cropped = image::imageops::crop_imm(&rgba, origin_x, origin_y, crop_w, crop_h).to_image();
        let fitted = image::imageops::resize(&cropped, width as u32, height as u32, image::imageops::FilterType::Lanczos3);
        for (index, sample) in fitted.pixels().enumerate() {
            let offset = index * 4;
            if offset + 3 >= pixels.len() {
                break;
            }
            pixels[offset] = sample[2];
            pixels[offset + 1] = sample[1];
            pixels[offset + 2] = sample[0];
            pixels[offset + 3] = 0xFF;
        }
    }

    #[derive(Clone, Copy)]
    enum Glyph { Play, Pause, Prev, Next, Heart, HeartFill }

    fn glyph_icon(kind: Glyph) -> windows::Win32::UI::WindowsAndMessaging::HICON {
        let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.clamp(20, 40);
        hicon_from_bgra(&draw_glyph(kind, size), size)
    }

    fn draw_glyph(kind: Glyph, size: i32) -> Vec<u8> {
        let step = 8i32;
        let big = size * step;
        let heart = matches!(kind, Glyph::Heart | Glyph::HeartFill);
        let zoom = if heart { 1.25f32 } else { 1.3f32 };
        // 0 = empty, 1 = outline, 2 = fill
        let mut mask = vec![0u8; (big * big) as usize];
        for y in 0..big {
            for x in 0..big {
                let u = (x as f32 + 0.5) / big as f32;
                let v = (y as f32 + 0.5) / big as f32;
                let su = 0.5 + (u - 0.5) / zoom;
                let sv = 0.5 + (v - 0.5) / zoom;
                let value = if heart {
                    if !heart_shape(su, sv, 0.0) {
                        0
                    } else if heart_shape(su, sv, 0.30) {
                        2
                    } else {
                        1
                    }
                } else if shape_covers(kind, su, sv) {
                    1
                } else {
                    0
                };
                mask[(y * big + x) as usize] = value;
            }
        }
        let outline = match kind {
            Glyph::HeartFill => (232u32, 84u32, 96u32),
            _ => (255u32, 255u32, 255u32),
        };
        let fill = match kind {
            Glyph::HeartFill => (232u32, 84u32, 96u32),
            Glyph::Heart => (255u32, 255u32, 255u32),
            _ => outline,
        };
        let mut pixels = vec![0u8; (size * size * 4) as usize];
        let samples = (step * step) as u32;
        for y in 0..size {
            for x in 0..size {
                let (mut line, mut body) = (0u32, 0u32);
                for oy in 0..step {
                    for ox in 0..step {
                        match mask[((y * step + oy) * big + x * step + ox) as usize] {
                            1 => line += 1,
                            2 => body += 1,
                            _ => {}
                        }
                    }
                }
                let coverage = (line + body) * 255 / samples;
                let index = ((y * size + x) * 4) as usize;
                pixels[index] = ((outline.2 * line + fill.2 * body) / samples) as u8;
                pixels[index + 1] = ((outline.1 * line + fill.1 * body) / samples) as u8;
                pixels[index + 2] = ((outline.0 * line + fill.0 * body) / samples) as u8;
                pixels[index + 3] = coverage as u8;
            }
        }
        pixels
    }

    fn shape_covers(kind: Glyph, u: f32, v: f32) -> bool {
        match kind {
            Glyph::Play => rounded_tri(u, v, (0.36, 0.26), (0.36, 0.74), (0.72, 0.50)),
            Glyph::Pause => rounded_box(u, v, 0.30, 0.42, 0.24, 0.76) || rounded_box(u, v, 0.58, 0.70, 0.24, 0.76),
            Glyph::Prev => {
                rounded_box(u, v, 0.24, 0.34, 0.24, 0.76) || rounded_tri(u, v, (0.70, 0.28), (0.70, 0.72), (0.42, 0.50))
            }
            Glyph::Next => {
                rounded_box(u, v, 0.66, 0.76, 0.24, 0.76) || rounded_tri(u, v, (0.30, 0.28), (0.30, 0.72), (0.58, 0.50))
            }
            Glyph::HeartFill => heart_shape(u, v, 0.0),
            Glyph::Heart => heart_shape(u, v, 0.0),
        }
    }

    /// The classic heart: a square turned 45 degrees with a round lobe on each upper side.
    /// `shrink` > 0 scales it toward the middle, which gives an even outline when subtracted.
    fn heart_shape(u: f32, v: f32, shrink: f32) -> bool {
        let scale = 1.0 - shrink;
        let (cx, cy) = (0.5f32, 0.52f32);
        let u = cx + (u - cx) / scale;
        let v = cy + (v - cy) / scale;
        let half = 0.2546f32;
        let square = (u - 0.5).abs() + (v - 0.56).abs() <= half;
        let radius = 0.18f32;
        let left = (u - 0.3725).powi(2) + (v - 0.4325).powi(2) <= radius * radius;
        let right = (u - 0.6275).powi(2) + (v - 0.4325).powi(2) <= radius * radius;
        square || left || right
    }

    fn rounded_box(u: f32, v: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> bool {
        let r = ((x1 - x0) / 2.0).min((y1 - y0) / 2.0);
        let cx = u.clamp(x0 + r, x1 - r);
        let cy = v.clamp(y0 + r, y1 - r);
        (u - cx).powi(2) + (v - cy).powi(2) <= r * r
    }

    fn rounded_tri(u: f32, v: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
        point_in_triangle(u, v, a, b, c) || edge_dist(u, v, a, b).min(edge_dist(u, v, b, c)).min(edge_dist(u, v, c, a)) <= 0.05
    }

    fn edge_dist(u: f32, v: f32, p: (f32, f32), q: (f32, f32)) -> f32 {
        let (dx, dy) = (q.0 - p.0, q.1 - p.1);
        let t = (((u - p.0) * dx + (v - p.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((u - (p.0 + t * dx)).powi(2) + (v - (p.1 + t * dy)).powi(2)).sqrt()
    }

    fn point_in_triangle(u: f32, v: f32, a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> bool {
        let sign = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| (p.0 - r.0) * (q.1 - r.1) - (q.0 - r.0) * (p.1 - r.1);
        let d1 = sign((u, v), a, b);
        let d2 = sign((u, v), b, c);
        let d3 = sign((u, v), c, a);
        !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
    }

    fn hicon_from_bgra(pixels: &[u8], size: i32) -> windows::Win32::UI::WindowsAndMessaging::HICON {
        unsafe {
            let screen = GetDC(None);
            let mut info = BITMAPINFO::default();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = size;
            info.bmiHeader.biHeight = -size;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let color = CreateDIBSection(Some(screen), &info, DIB_RGB_COLORS, &mut bits, None, 0).unwrap_or_default();
            if !bits.is_null() {
                std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len().min((size * size * 4) as usize));
            }
            let stride = ((size + 15) / 16) * 2;
            let mut mask_bits = vec![0xFFu8; (stride * size) as usize];
            for y in 0..size {
                for x in 0..size {
                    let alpha = pixels.get(((y * size + x) * 4 + 3) as usize).copied().unwrap_or(0);
                    if alpha > 32 {
                        let index = (y * stride + x / 8) as usize;
                        mask_bits[index] &= !(0x80u8 >> (x % 8));
                    }
                }
            }
            let mask = CreateBitmap(size, size, 1, 1, Some(mask_bits.as_ptr().cast()));
            let icon_info = ICONINFO { fIcon: true.into(), xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
            let icon = CreateIconIndirect(&icon_info).unwrap_or_default();
            let _ = DeleteObject(HGDIOBJ(color.0));
            let _ = DeleteObject(HGDIOBJ(mask.0));
            ReleaseDC(None, screen);
            icon
        }
    }

}

