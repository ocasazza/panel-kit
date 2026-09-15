//! Browser surface measurement adapters for host-owned workspaces.

mod viewport_observer;

use panel_kit_core::reducer::Viewport;
use panel_kit_core::{SurfaceCapabilities, SurfaceProfile, Units, WEB_COMPACT_MAX, WEB_TABLET_MAX};

pub use viewport_observer::{observe_viewport, ViewportObserverState, ViewportObserverStatus};

/// Current browser viewport size in CSS pixels, with host-test fallbacks.
pub fn viewport_size() -> (f64, f64) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        (1280.0, 800.0)
    }

    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window();
        let width = window
            .as_ref()
            .and_then(|window| window.inner_width().ok())
            .and_then(|value| value.as_f64())
            .unwrap_or(1280.0);
        let height = window
            .and_then(|window| window.inner_height().ok())
            .and_then(|value| value.as_f64())
            .unwrap_or(800.0);
        (width, height)
    }
}

/// Convert raw dimensions into a core viewport, rejecting unsafe values.
pub fn viewport_from_size(width: f64, height: f64) -> Option<Viewport> {
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        Some(Viewport { width, height, units: Units::CssPx })
    } else {
        None
    }
}

/// Browser input capabilities detected from media queries.
pub fn browser_capabilities() -> SurfaceCapabilities {
    #[cfg(not(target_arch = "wasm32"))]
    {
        SurfaceCapabilities { coarse_pointer: false, hover: true, keyboard: true }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let media_matches = |query: &str| {
            web_sys::window()
                .and_then(|window| window.match_media(query).ok().flatten())
                .map(|media| media.matches())
                .unwrap_or(false)
        };
        SurfaceCapabilities {
            coarse_pointer: media_matches("(pointer: coarse)"),
            hover: media_matches("(hover: hover)"),
            keyboard: true,
        }
    }
}

/// Classify a browser width into panel-kit's web surface profile.
pub fn surface_profile(width: f64) -> SurfaceProfile {
    SurfaceProfile::from_logical_width(
        width,
        WEB_COMPACT_MAX,
        WEB_TABLET_MAX,
        browser_capabilities(),
    )
}
