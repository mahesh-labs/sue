//! Sue's audited bridge to `winit`'s Wayland display and window surface.
//!
//! `winit` does not bind `zwp_tablet_manager_v2`, so pen displays and drawing
//! tablets on Wayland need a dedicated `wl_event_queue` on the same
//! `wl_display` as the window. Wrapping `winit`'s raw `wl_display` and
//! `wl_surface` pointers requires two `unsafe` calls into `wayland-backend`,
//! isolated here (part of `sue-platform`, backbone rule 6) while keeping an
//! `Arc<Window>` alive so the underlying Wayland objects outlive the queue.

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
))]
mod imp {
    use std::sync::Arc;

    use wayland_backend::client::{Backend, ObjectId};
    use wayland_client::protocol::wl_surface::WlSurface;
    use wayland_client::{Connection, Proxy};
    use winit::raw_window_handle::{
        HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
    };
    use winit::window::Window;

    /// A `wayland-client` connection sharing `winit`'s `wl_display`, paired
    /// with the Wayland object id of `window`'s `wl_surface`.
    ///
    /// Field drop order matters: `connection` drops before `_window`, so our
    /// `wl_event_queue` is destroyed while `winit`'s `wl_display` is still
    /// open.
    pub struct WindowConnection {
        connection: Connection,
        surface_id: u32,
        _window: Arc<Window>,
    }

    impl WindowConnection {
        /// Opens a guest Wayland connection on `window`'s display when `window`
        /// is a Wayland window, or returns `None` on X11 and other backends.
        #[must_use]
        #[allow(
            unsafe_code,
            reason = "wraps winit's live wl_display and wl_surface pointers; approved 2026-11-05"
        )]
        pub fn open(window: &Arc<Window>) -> Option<Self> {
            let display_handle = window.display_handle().ok()?;
            let window_handle = window.window_handle().ok()?;
            let (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(surface)) =
                (display_handle.as_raw(), window_handle.as_raw())
            else {
                return None;
            };

            // SAFETY: `surface.surface` comes from `window.window_handle()` on
            // a live `winit::window::Window`, so it is a valid, non-null
            // `*mut wl_surface` (`*mut wl_proxy`) whose class matches
            // `WlSurface::interface()` for the duration of this call. We only
            // extract its numeric Wayland protocol id (`u32`).
            let surface_id = unsafe {
                ObjectId::from_ptr(WlSurface::interface(), surface.surface.as_ptr().cast())
            }
            .ok()?
            .protocol_id();

            // SAFETY: `display.display` comes from `window.display_handle()` on
            // `window`, so it is a valid `*mut wl_display` connected by
            // `winit`. `Self` retains an `Arc<Window>` in `_window` declared
            // after `connection`, guaranteeing the `wl_display` outlives
            // `connection` and every Wayland queue created from it.
            let backend = unsafe { Backend::from_foreign_display(display.display.as_ptr().cast()) };
            let connection = Connection::from_backend(backend);

            Some(Self {
                connection,
                surface_id,
                _window: Arc::clone(window),
            })
        }

        /// The shared Wayland client connection.
        #[must_use]
        pub fn connection(&self) -> &Connection {
            &self.connection
        }

        /// The Wayland protocol object id (`wl_surface@...`) of the window.
        #[must_use]
        pub fn surface_id(&self) -> u32 {
            self.surface_id
        }
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
))]
pub use imp::WindowConnection;
