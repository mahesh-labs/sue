//! A pen's pressure, tilt and eraser state where the window's touch events do
//! not carry them: a WinTab tablet on Windows (tablet drivers with Windows Ink
//! turned off, common among artists), a tablet on Linux and the BSDs (Wayland's
//! `tablet-v2` protocol and X11's XInput2, since `winit` carries neither) and
//! a pen in a browser, where `winit` treats pens as mice. With Windows Ink, on
//! macOS and on phones and tablets, pressure arrives with egui's touch events
//! instead.
//!
//! The host updates this state every frame; studios read [`take_samples`] for
//! per-event samples when the host queues them (Wayland's 200 Hz tablet stream)
//! and fall back to [`pressure`], [`tilt`] and [`eraser`].

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

/// Where [`pressure`] and [`tilt`] come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenSource {
    /// A WinTab tablet driver (Windows).
    WinTab,
    /// A pen's pointer events in a browser.
    Browser,
    /// A tablet on Wayland's `tablet-v2` protocol (Linux and BSDs).
    Wayland,
    /// A tablet on X11's XInput2 extension (Linux and BSDs).
    X11,
}

/// Which pointer event a queued [`Sample`] belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleKind {
    /// `egui::Event::PointerMoved`.
    Move,
    /// `egui::Event::PointerButton` with `pressed: true`.
    Press,
    /// `egui::Event::PointerButton` with `pressed: false`.
    Release,
}

/// One stylus reading paired with an `egui` pointer event in the current frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// Which kind of pointer event this reading accompanies.
    pub kind: SampleKind,
    /// Pointer position in `egui` points (`[x, y]`).
    pub pos: [f32; 2],
    /// Normalized contact pressure (`0.0..=1.0`), or `None` while hovering or
    /// lifting.
    pub pressure: Option<f32>,
    /// Pen tilt `[tilt_x, tilt_y]` in degrees from upright (`-90.0..=90.0`).
    pub tilt: [f32; 2],
    /// `true` when the pen's eraser end or eraser switch is active.
    pub eraser: bool,
}

/// Maximum queued samples kept within one frame.
const MAX_FRAME_SAMPLES: usize = 512;

/// The pressure's bits, or `u32::MAX` while no pen touches.
static PRESSURE: AtomicU32 = AtomicU32::new(u32::MAX);
/// `tilt_x` in degrees as `f32` bits.
static TILT_X: AtomicU32 = AtomicU32::new(0);
/// `tilt_y` in degrees as `f32` bits.
static TILT_Y: AtomicU32 = AtomicU32::new(0);
/// Whether the eraser end of the pen is active.
static ERASER: AtomicBool = AtomicBool::new(false);
/// The last [`PenSource`] seen: 0 none, 1 WinTab, 2 Browser, 3 Wayland, 4 X11.
static SOURCE: AtomicU8 = AtomicU8::new(0);

/// Per-event samples emitted for the current frame, in FIFO order.
static SAMPLES: Mutex<VecDeque<Sample>> = Mutex::new(VecDeque::new());

/// The pen's pressure, 0 to 1, while it touches; `None` when no such pen
/// touches.
#[must_use]
pub fn pressure() -> Option<f32> {
    let bits = PRESSURE.load(Ordering::Relaxed);
    (bits != u32::MAX).then(|| f32::from_bits(bits))
}

/// The pen's tilt `[tilt_x, tilt_y]` in degrees (`-90.0..=90.0`), or
/// `[0.0, 0.0]` when upright or unreported.
#[must_use]
pub fn tilt() -> [f32; 2] {
    [
        f32::from_bits(TILT_X.load(Ordering::Relaxed)),
        f32::from_bits(TILT_Y.load(Ordering::Relaxed)),
    ]
}

/// `true` while the pen's eraser end is in use.
#[must_use]
pub fn eraser() -> bool {
    ERASER.load(Ordering::Relaxed)
}

/// Where pen pressure last came from, once a pen has been seen.
#[must_use]
pub fn source() -> Option<PenSource> {
    match SOURCE.load(Ordering::Relaxed) {
        1 => Some(PenSource::WinTab),
        2 => Some(PenSource::Browser),
        3 => Some(PenSource::Wayland),
        4 => Some(PenSource::X11),
        _ => None,
    }
}

/// Takes all per-event stylus samples queued for the current frame.
#[must_use]
pub fn take_samples() -> VecDeque<Sample> {
    let Ok(mut guard) = SAMPLES.lock() else {
        return VecDeque::new();
    };
    std::mem::take(&mut *guard)
}

/// Records only the pen's pressure (for sources such as WinTab that do not
/// report tilt or eraser state here).
#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
pub(crate) fn set(from: PenSource, pressure: Option<f32>) {
    set_state(from, pressure, [0.0, 0.0], false);
}

/// Records the pen's latest pressure, tilt (in degrees) and eraser state.
#[cfg_attr(
    not(any(
        target_os = "windows",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd",
        target_arch = "wasm32",
    )),
    allow(dead_code)
)]
pub(crate) fn set_state(from: PenSource, pressure: Option<f32>, tilt: [f32; 2], eraser: bool) {
    SOURCE.store(
        match from {
            PenSource::WinTab => 1,
            PenSource::Browser => 2,
            PenSource::Wayland => 3,
            PenSource::X11 => 4,
        },
        Ordering::Relaxed,
    );
    let bits = pressure.map_or(u32::MAX, |value| value.clamp(0.0, 1.0).to_bits());
    PRESSURE.store(bits, Ordering::Relaxed);
    let clamp_deg = |deg: f32| {
        if deg.is_finite() {
            deg.clamp(-90.0, 90.0)
        } else {
            0.0
        }
    };
    TILT_X.store(clamp_deg(tilt[0]).to_bits(), Ordering::Relaxed);
    TILT_Y.store(clamp_deg(tilt[1]).to_bits(), Ordering::Relaxed);
    ERASER.store(eraser, Ordering::Relaxed);
}

/// Records a per-event [`Sample`] alongside the `egui::Event` emitted for it,
/// and updates the latest atomic state.
#[cfg_attr(
    not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd",
    )),
    allow(dead_code)
)]
pub(crate) fn push_sample(from: PenSource, mut sample: Sample) {
    sample.pressure = sample.pressure.map(|value| value.clamp(0.0, 1.0));
    sample.tilt = sample.tilt.map(|deg| {
        if deg.is_finite() {
            deg.clamp(-90.0, 90.0)
        } else {
            0.0
        }
    });
    set_state(from, sample.pressure, sample.tilt, sample.eraser);
    if let Ok(mut guard) = SAMPLES.lock() {
        if guard.len() >= MAX_FRAME_SAMPLES {
            guard.pop_front();
        }
        guard.push_back(sample);
    }
}

/// Drops any queued samples not consumed during the frame that just ran.
#[cfg_attr(
    not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "dragonfly",
        target_os = "netbsd",
        target_os = "openbsd",
    )),
    allow(dead_code)
)]
pub(crate) fn end_frame() {
    if let Ok(mut guard) = SAMPLES.lock() {
        guard.clear();
    }
}

/// Serializes unit tests in this crate that inspect process-wide stylus statics.
#[cfg(test)]
pub(crate) fn lock_for_test() -> std::sync::MutexGuard<'static, ()> {
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pens_pressure_tilt_and_eraser_read_back_until_it_lifts() {
        let _guard = lock_for_test();
        set(PenSource::WinTab, Some(0.42));
        assert_eq!(pressure(), Some(0.42));
        assert_eq!(tilt(), [0.0, 0.0]);
        assert!(!eraser());
        assert_eq!(source(), Some(PenSource::WinTab));

        set_state(PenSource::X11, Some(3.0), [35.0, -120.0], true);
        assert_eq!(pressure(), Some(1.0), "clamped");
        assert_eq!(tilt(), [35.0, -90.0], "tilt clamped to [-90, 90]");
        assert!(eraser());
        assert_eq!(source(), Some(PenSource::X11));

        set_state(PenSource::Browser, None, [0.0, 0.0], false);
        assert_eq!(pressure(), None);
        assert_eq!(tilt(), [0.0, 0.0]);
        assert!(!eraser());
        assert_eq!(source(), Some(PenSource::Browser));
    }

    #[test]
    fn queued_samples_preserve_per_event_readings_and_clear_at_frame_end() {
        let _guard = lock_for_test();
        end_frame();
        push_sample(
            PenSource::Wayland,
            Sample {
                kind: SampleKind::Press,
                pos: [10.0, 20.0],
                pressure: Some(0.3),
                tilt: [15.0, -10.0],
                eraser: true,
            },
        );
        push_sample(
            PenSource::Wayland,
            Sample {
                kind: SampleKind::Release,
                pos: [10.0, 20.0],
                pressure: None,
                tilt: [15.0, -10.0],
                eraser: true,
            },
        );
        // Even though the release cleared the latest atomic pressure, the
        // queued Press sample still carries its 0.3 contact pressure.
        assert_eq!(pressure(), None);
        let samples = take_samples();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0].kind, SampleKind::Press);
        assert_eq!(samples[0].pressure, Some(0.3));
        assert_eq!(samples[0].tilt, [15.0, -10.0]);
        assert!(samples[0].eraser);
        assert_eq!(samples[1].kind, SampleKind::Release);
        assert_eq!(samples[1].pressure, None);

        push_sample(
            PenSource::Wayland,
            Sample {
                kind: SampleKind::Move,
                pos: [1.0, 2.0],
                pressure: Some(0.5),
                tilt: [0.0, 0.0],
                eraser: false,
            },
        );
        end_frame();
        assert!(take_samples().is_empty());
        set(PenSource::Wayland, None);
    }
}
