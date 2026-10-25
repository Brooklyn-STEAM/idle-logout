#![windows_subsystem = "windows"]

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::progress::ProgressCircle;
use gpui_kit::component::Theme;
use gpui_kit::*;
use std::borrow::Cow;
use std::time::{Duration, Instant};

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

fn phase(idle: Duration, idle_after: Duration, countdown: Duration) -> Phase {
    if idle < idle_after {
        Phase::Hidden
    } else if idle < idle_after + countdown {
        Phase::Countdown(idle_after + countdown - idle)
    } else {
        Phase::Logout
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
}

impl Render for Panel {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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
                        .child(div().text_size(px(14.)).line_height(px(20.)).text_color(rgb(INK_MUTED)).child("Move the mouse or press a key to stay signed in.")),
                )
                .child(
                    Button::new("now")
                        .primary()
                        .rounded(px(12.)) // radius-md
                        .font_family(DISPLAY)
                        .text_size(px(20.))
                        .label("SIGN OUT")
                        .on_click(move |_, _, cx| {
                            log_out(dry_run);
                            cx.quit();
                        }),
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
        focus: false,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    open_window(options, cx, |_, cx| cx.new(|_| Panel { remaining, total, dry_run }))
        .inspect_err(|e| eprintln!("could not open panel: {e:#}"))
        .ok()
}

fn main() {
    let dry_run = std::env::args().any(|a| a == "--dry-run");
    let (idle_after, countdown) = if dry_run {
        (Duration::from_secs(5), Duration::from_secs(10))
    } else {
        (Duration::from_secs(60), Duration::from_secs(240))
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
            cx.spawn(async move |cx| {
                let mut panel: Option<(AnyWindowHandle, Entity<Panel>)> = None;
                loop {
                    cx.background_executor().timer(Duration::from_millis(250)).await;
                    let now = phase(idle_time(started), idle_after, countdown);
                    cx.update(|cx| match now {
                        Phase::Hidden => {
                            if let Some((window, _)) = panel.take() {
                                window.update(cx, |_, window, _| window.remove_window()).ok();
                            }
                        }
                        Phase::Countdown(remaining) => match &panel {
                            Some((_, view)) => view.update(cx, |p, cx| {
                                p.remaining = remaining;
                                cx.notify();
                            }),
                            None => panel = open_panel(remaining, countdown, dry_run, cx),
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
    use super::{phase, Phase};
    use std::time::Duration;

    #[test]
    fn phases() {
        let s = Duration::from_secs;
        let p = |idle| phase(s(idle), s(60), s(240));
        assert_eq!(p(0), Phase::Hidden);
        assert_eq!(p(59), Phase::Hidden);
        assert_eq!(p(60), Phase::Countdown(s(240)));
        assert_eq!(p(299), Phase::Countdown(s(1)));
        assert_eq!(p(300), Phase::Logout);
    }
}
