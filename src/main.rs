#![windows_subsystem = "windows"]

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::progress::ProgressCircle;
use gpui_kit::component::Theme;
use gpui_kit::*;
use std::borrow::Cow;
use std::time::{Duration, Instant};

#[cfg(windows)]
mod service;

const PANEL: Size<Pixels> = size(px(560.), px(152.));

// Brooklyn STEAM Center design system tokens (light theme).
const BLUE: u32 = 0x004bb6;
const WHITE: u32 = 0xffffff;
const BLACK: u32 = 0x000000;
const INK_MUTED: u32 = 0x5c5d60;
const LINE: u32 = 0xc9cacc;
const DISPLAY: &str = "Dosis";
// The brand's Helvetica stack; Windows has only Arial from it.
const BODY: &str = "Arial";

fn hsla_from(hex: u32) -> Hsla {
    rgb(hex).into()
}
// Leaves room for a standard Windows taskbar.
const BOTTOM_MARGIN: f32 = 64.;

#[derive(Debug, PartialEq)]
enum Phase {
    Hidden,
    Countdown(Duration),
    Logout,
}

// Once shown, the countdown ignores activity; only "Stay signed in" stops it.
fn phase(idle: Duration, shown_for: Option<Duration>, idle_after: Duration, countdown: Duration) -> Phase {
    match shown_for {
        None if idle < idle_after => Phase::Hidden,
        None => Phase::Countdown(countdown),
        Some(shown) if shown < countdown => Phase::Countdown(countdown - shown),
        Some(_) => Phase::Logout,
    }
}

#[cfg(windows)]
fn idle_time(_started: Instant) -> Duration {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // Tick counts wrap every ~49 days; wrapping_sub keeps the difference correct.
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return Duration::ZERO;
        }
        Duration::from_millis(GetTickCount().wrapping_sub(info.dwTime) as u64)
    }
}

// shortcut: no system idle query off Windows, so time since launch stands in; only for previewing the UI.
#[cfg(not(windows))]
fn idle_time(started: Instant) -> Duration {
    started.elapsed()
}

#[cfg(windows)]
fn tray() -> Option<tray_icon::TrayIcon> {
    use tray_icon::menu::{Menu, MenuItem};
    let label = concat!("Idle Logout v", env!("CARGO_PKG_VERSION"));
    let menu = Menu::new();
    menu.append(&MenuItem::new(label, false, None)).ok()?;
    tray_icon::TrayIconBuilder::new()
        .with_tooltip(label)
        .with_menu(Box::new(menu))
        .with_icon(tray_icon::Icon::from_rgba(clock_rgba(32), 32, 32).ok()?)
        .build()
        .inspect_err(|e| eprintln!("could not add tray icon: {e}"))
        .ok()
}

// White clock hands on a brand-blue disc, drawn here so no icon file needs bundling.
// Laid out on a 24-unit grid like the brand's Lucide icons, with 4x4 supersampling for smooth edges.
#[cfg_attr(not(windows), allow(dead_code))]
fn clock_rgba(size: u32) -> Vec<u8> {
    // Distance from p to the hand running from the centre (12, 12) to end.
    let hand = |p: (f32, f32), end: (f32, f32)| {
        let (dx, dy) = (end.0 - 12., end.1 - 12.);
        let t = (((p.0 - 12.) * dx + (p.1 - 12.) * dy) / (dx * dx + dy * dy)).clamp(0., 1.);
        (p.0 - 12. - t * dx).hypot(p.1 - 12. - t * dy)
    };
    let unit = 24. / size as f32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for i in 0..size * size {
        let (mut disc, mut white) = (0., 0.);
        for s in 0..16 {
            let x = ((i % size) as f32 + (s % 4) as f32 / 4. + 0.125) * unit;
            let y = ((i / size) as f32 + (s / 4) as f32 / 4. + 0.125) * unit;
            if (x - 12.).hypot(y - 12.) <= 12. {
                disc += 1. / 16.;
                if hand((x, y), (12., 5.5)) <= 1.4 || hand((x, y), (16.5, 14.5)) <= 1.4 {
                    white += 1. / 16.;
                }
            }
        }
        let mix = |c: u8| (c as f32 + (255. - c as f32) * if disc > 0. { white / disc } else { 0. }) as u8;
        rgba.extend([mix(0x00), mix(0x4b), mix(0xb6), (disc * 255.) as u8]);
    }
    rgba
}

// shortcut: tray only on Windows; tray-icon needs GTK elsewhere, add it if the UI preview needs one.
#[cfg(not(windows))]
fn tray() -> Option<()> {
    None
}

// Machine-wide only (HKLM), so admins or Group Policy set it and users can't override it.
#[cfg(windows)]
fn config_secs(name: &str, default: u64) -> Duration {
    use windows_sys::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RegGetValueW};
    let wide = |s: &str| s.encode_utf16().chain([0]).collect::<Vec<u16>>();
    let (mut value, mut size) = (0u32, size_of::<u32>() as u32);
    let found = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            wide(r"Software\Idle Logout").as_ptr(),
            wide(name).as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut value).cast(),
            &mut size,
        )
    } == 0;
    // 0 would sign people out instantly, so treat it like a missing value.
    Duration::from_secs(if found && value > 0 { value as u64 } else { default })
}

#[cfg(not(windows))]
fn config_secs(_: &str, default: u64) -> Duration {
    Duration::from_secs(default)
}

// gpui creates popups with style 0, which Windows treats as a captioned window: DWM then draws a
// 1px top edge, a border and a square shadow outside the card's rounded corners. A bare WS_POPUP has none.
#[cfg(windows)]
fn hide_frame(window: &impl raw_window_handle::HasWindowHandle) {
    use raw_window_handle::RawWindowHandle;
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DwmSetWindowAttribute,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
        WS_CAPTION, WS_POPUP, WS_THICKFRAME,
    };
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else { return };
    let hwnd = handle.hwnd.get() as _;
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32 & !(WS_CAPTION | WS_THICKFRAME) | WS_POPUP;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as _);
        SetWindowPos(hwnd, std::ptr::null_mut(), 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
        DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR as _, std::ptr::from_ref(&DWMWA_COLOR_NONE).cast(), 4);
        DwmSetWindowAttribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE as _, std::ptr::from_ref(&DWMWCP_DONOTROUND).cast(), 4);
    }
}

#[cfg(not(windows))]
fn hide_frame(_: &Window) {}

// The system's own notification sound, so it follows the user's sound scheme and mute.
#[cfg(windows)]
fn chime() {
    use windows_sys::Win32::System::Diagnostics::Debug::MessageBeep;
    use windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONINFORMATION;
    unsafe { MessageBeep(MB_ICONINFORMATION) };
}

#[cfg(not(windows))]
fn chime() {}

fn log_out(dry_run: bool) {
    if dry_run {
        println!("would log out now");
    } else if let Err(e) = std::process::Command::new("shutdown").arg("/l").spawn() {
        eprintln!("logout failed: {e}");
    }
}

struct Panel {
    remaining: Duration,
    total: Duration,
    dry_run: bool,
    stay: bool,
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let secs = self.remaining.as_secs_f32().ceil() as u64;
        let pct = 100. * self.remaining.as_secs_f32() / self.total.as_secs_f32();
        let dry_run = self.dry_run;
        let blue = rgb(BLUE);

        div().size_full().overflow_hidden().font_family(BODY).child(
            div()
                .absolute()
                .size_full()
                .flex()
                .items_center()
                .gap(px(24.)) // space-5
                .p(px(24.)) // space-5
                .bg(rgb(WHITE))
                .border_1() // stroke-hairline
                .border_color(rgb(LINE))
                .rounded(px(16.)) // radius-lg
                .child(
                    div().flex_none().size(px(64.)).child(
                        ProgressCircle::new("left")
                            .value(pct)
                            .color(blue)
                            .size_full()
                            .accessibility_label("Time until sign out"),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        // display-sm eyebrow
                        .child(div().font_family(DISPLAY).text_size(px(20.)).line_height(px(24.)).text_color(blue).child("SIGNING OUT IN"))
                        // display-lg countdown
                        .child(div().font_family(DISPLAY).text_size(px(48.)).line_height(px(52.)).text_color(rgb(BLACK)).child(format!("{}:{:02}", secs / 60, secs % 60)))
                        // body-sm helper
                        .child(div().text_size(px(14.)).line_height(px(20.)).text_color(rgb(INK_MUTED)).child("Still here? Choose to stay signed in.")),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.)) // space-2
                        .child(
                            Button::new("now")
                                .primary()
                                .rounded(px(12.)) // radius-md
                                .font_family(DISPLAY)
                                .text_size(px(20.))
                                .w_full()
                                .cursor_pointer()
                                .label("SIGN OUT")
                                .on_click(move |_, _, cx| {
                                    log_out(dry_run);
                                    cx.quit();
                                }),
                        )
                        .child(
                            Button::new("stay")
                                .ghost()
                                // Light brand tint so it reads as a button without competing with Sign out.
                                .bg(blue.opacity(0.1))
                                .text_color(blue)
                                .rounded(px(12.)) // radius-md
                                .font_family(DISPLAY)
                                .text_size(px(20.))
                                .w_full()
                                .cursor_pointer()
                                .label("STAY SIGNED IN")
                                .on_click(cx.listener(|p, _, _, _| p.stay = true)),
                        ),
                )
                .with_animation(
                    "slide-in",
                    Animation::new(Duration::from_millis(350)).with_easing(ease_out_quint()),
                    |el, t| el.top(PANEL.height * (1. - t)),
                ),
        )
    }
}

fn open_panel(remaining: Duration, total: Duration, dry_run: bool, cx: &mut App) -> Option<(AnyWindowHandle, Entity<Panel>)> {
    let screen = cx.primary_display()?.bounds();
    let origin = point(
        screen.center().x - PANEL.width / 2.,
        screen.bottom() - PANEL.height - px(BOTTOM_MARGIN),
    );
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, PANEL))),
        titlebar: None,
        focus: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    open_window(options, cx, |window, cx| {
        // Windows won't let a background app take the foreground from `focus` alone; this forces it.
        hide_frame(window);
        window.activate_window();
        cx.new(|_| Panel { remaining, total, dry_run, stay: false })
    })
        .inspect_err(|e| eprintln!("could not open panel: {e:#}"))
        .ok()
}

fn main() {
    #[cfg(windows)]
    if std::env::args().any(|a| a == "--service") {
        return service::run();
    }
    let dry_run = std::env::args().any(|a| a == "--dry-run");
    let (idle_after, countdown) = if dry_run {
        (Duration::from_secs(5), Duration::from_secs(10))
    } else {
        (config_secs("IdleSeconds", 60), config_secs("CountdownSeconds", 240))
    };
    let started = Instant::now();

    application()
        .with_assets(assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            init(cx);
            // Primary buttons in brand blue with on-blue text.
            Theme::update(cx, |t| {
                // Root paints this behind the whole window; clear it so the card's rounded corners show through.
                t.colors.background = transparent_black();
                let blue = hsla_from(BLUE);
                t.colors.button_primary = blue;
                t.colors.button_primary_hover = blue.opacity(0.9);
                t.colors.button_primary_active = blue.opacity(0.8);
                t.colors.button_primary_foreground = hsla_from(WHITE);
            });
            cx.text_system()
                .add_fonts(vec![Cow::Borrowed(include_bytes!("../assets/Dosis-Regular.ttf"))])
                .expect("bundled Dosis font loads");
            // Created on the UI thread, whose message loop drives it; dropping it removes the icon.
            let tray = tray();
            cx.spawn(async move |cx| {
                let _tray = tray;
                let mut panel: Option<(AnyWindowHandle, Entity<Panel>, Instant)> = None;
                // Also counts as activity, so the idle clock restarts even where idle_time can't see input.
                let mut stayed_at = started;
                loop {
                    cx.background_executor().timer(Duration::from_millis(250)).await;
                    let idle = idle_time(started).min(stayed_at.elapsed());
                    let now = phase(idle, panel.as_ref().map(|(_, _, shown)| shown.elapsed()), idle_after, countdown);
                    cx.update(|cx| match now {
                        Phase::Hidden => {}
                        Phase::Countdown(remaining) => match &panel {
                            Some((window, view, _)) if view.read(cx).stay => {
                                window.update(cx, |_, window, _| window.remove_window()).ok();
                                panel = None;
                                stayed_at = Instant::now();
                            }
                            Some((_, view, _)) => view.update(cx, |p, cx| {
                                p.remaining = remaining;
                                cx.notify();
                            }),
                            None => {
                                panel = open_panel(remaining, countdown, dry_run, cx).map(|(w, v)| (w, v, Instant::now()));
                                chime();
                            }
                        },
                        Phase::Logout => {
                            log_out(dry_run);
                            cx.quit();
                        }
                    });
                    if now == Phase::Logout {
                        break;
                    }
                }
            })
            .detach();
        });
}

#[cfg(test)]
mod tests {
    use super::{clock_rgba, phase, Phase};
    use std::time::Duration;

    #[test]
    fn clock_icon() {
        let px = |x: usize, y: usize| clock_rgba(32)[(y * 32 + x) * 4..][..4].to_vec();
        assert_eq!(px(0, 0)[3], 0, "corner is transparent");
        assert_eq!(px(16, 16), [255, 255, 255, 255], "hands meet white at the centre");
        assert_eq!(px(5, 16), [0x00, 0x4b, 0xb6, 255], "face is brand blue");
    }

    #[test]
    fn phases() {
        let s = Duration::from_secs;
        let p = |idle, shown: Option<u64>| phase(s(idle), shown.map(s), s(60), s(240));
        assert_eq!(p(0, None), Phase::Hidden);
        assert_eq!(p(59, None), Phase::Hidden);
        assert_eq!(p(60, None), Phase::Countdown(s(240)));
        // Activity after the panel shows doesn't stop the countdown.
        assert_eq!(p(0, Some(10)), Phase::Countdown(s(230)));
        assert_eq!(p(0, Some(239)), Phase::Countdown(s(1)));
        assert_eq!(p(0, Some(240)), Phase::Logout);
    }
}
