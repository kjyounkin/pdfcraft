//! Stylus mode for pen tablets in the browser: the pen uses the chosen tool, a finger moves the
//! page, and a palm resting on the screen while the pen writes is ignored.
//!
//! eframe passes pen and finger input on as the same pointer, so [`install`] listens ahead of it
//! (capture phase on the window) for the browser's `pointerType`. Stylus mode turns on the first
//! time a pen is seen and is remembered for later visits; `?stylus=off` keeps it off and
//! `?stylus=on` starts with it on.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::QuickTool;

/// A pen has been used: fingers move the page and palms are ignored while the pen is near.
static ENABLED: AtomicBool = AtomicBool::new(false);
/// The latest press came from a finger (not a pen or mouse).
static FINGER_PRESS: AtomicBool = AtomicBool::new(false);

/// Whether stylus mode is on.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// The tool the page view acts with: the Hand tool while a finger drags with a tool the pen
/// should own, otherwise the chosen tool. The toolbar and shortcuts keep the chosen tool.
pub(crate) fn page_tool(chosen: QuickTool) -> QuickTool {
    let pen_owned = matches!(
        chosen,
        QuickTool::Select
            | QuickTool::Comment(_)
            | QuickTool::Redact
            | QuickTool::ColumnSelect
            | QuickTool::Measure(_)
            | QuickTool::MarqueeZoom
            | QuickTool::Snapshot
    );
    if pen_owned && enabled() && FINGER_PRESS.load(Ordering::Relaxed) { QuickTool::Hand } else { chosen }
}

/// Start listening for pen and touch input. Call once, before the app starts.
pub fn install() {
    #[cfg(target_arch = "wasm32")]
    web::install();
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::cell::RefCell;
    use std::sync::atomic::Ordering;

    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use web_sys::{AddEventListenerOptions, Event, PointerEvent, TouchEvent};

    use super::{ENABLED, FINGER_PRESS};

    /// A touch while the pen is down or this soon after it lifts is a palm. Android already
    /// ignores a palm resting on the screen, and a Pixel Tablet showed deliberate finger swipes
    /// about half a second after the pen, so the window only covers the pen being lifted.
    const PEN_GRACE_MS: f64 = 300.0;
    /// A palm's `touchstart` follows its `pointerdown` within this long.
    const SAME_CONTACT_MS: f64 = 100.0;
    const STORAGE_KEY: &str = "pdfcraft-stylus";

    #[derive(Default)]
    struct State {
        pen_down: bool,
        /// `performance.now()` of the last pen lift.
        pen_lift: f64,
        /// When the latest palm `pointerdown` was dropped, until its `touchstart` arrives.
        palm_pressed: Option<f64>,
        /// Touches set aside as palms: every later event of theirs is dropped too.
        palm_pointers: Vec<i32>,
        palm_touches: Vec<i32>,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    fn now() -> f64 {
        web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now())
    }

    fn pen_near(s: &State) -> bool {
        s.pen_down || now() - s.pen_lift < PEN_GRACE_MS
    }

    fn drop_event(e: &Event) {
        e.stop_immediate_propagation();
        e.prevent_default();
    }

    fn remember_enabled(window: &web_sys::Window) {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.set_item(STORAGE_KEY, "1");
        }
    }

    fn on_pointer(e: PointerEvent) {
        let kind = e.pointer_type();
        let ty = e.type_();
        let (down, up) = (ty == "pointerdown", ty == "pointerup" || ty == "pointercancel");
        STATE.with_borrow_mut(|s| match kind.as_str() {
            "pen" => {
                if down {
                    s.pen_down = true;
                    FINGER_PRESS.store(false, Ordering::Relaxed);
                } else if up {
                    s.pen_down = false;
                    s.pen_lift = now();
                }
                if !ENABLED.swap(true, Ordering::Relaxed)
                    && let Some(w) = web_sys::window()
                {
                    remember_enabled(&w);
                }
            }
            "touch" if ENABLED.load(Ordering::Relaxed) => {
                let id = e.pointer_id();
                if let Some(i) = s.palm_pointers.iter().position(|p| *p == id) {
                    if up {
                        s.palm_pointers.swap_remove(i);
                    }
                    drop_event(&e);
                } else if down {
                    if pen_near(s) {
                        s.palm_pointers.push(id);
                        s.palm_pressed = Some(now());
                        drop_event(&e);
                    } else {
                        FINGER_PRESS.store(true, Ordering::Relaxed);
                    }
                }
            }
            "touch" if down => FINGER_PRESS.store(true, Ordering::Relaxed),
            _ if down => FINGER_PRESS.store(false, Ordering::Relaxed),
            _ => {}
        });
    }

    /// eframe also reads touch events; drop the ones made only by palms. The pen makes touch
    /// events too, and Chrome on Android leaves their `touchType` unset, so a touch counts as a
    /// palm only when it starts right after a palm's `pointerdown` (browsers send that first).
    fn on_touch(e: TouchEvent) {
        if !ENABLED.load(Ordering::Relaxed) {
            return;
        }
        let ty = e.type_();
        let changed = e.changed_touches();
        let ids: Vec<i32> = (0..changed.length()).filter_map(|i| changed.get(i)).map(|t| t.identifier()).collect();
        if ids.is_empty() {
            return;
        }
        STATE.with_borrow_mut(|s| {
            let palms = if ty == "touchstart" {
                s.palm_pressed.take().is_some_and(|at| now() - at < SAME_CONTACT_MS)
            } else {
                ids.iter().all(|id| s.palm_touches.contains(id))
            };
            if !palms {
                return;
            }
            if ty == "touchstart" {
                s.palm_touches.extend(&ids);
            } else if ty == "touchend" || ty == "touchcancel" {
                s.palm_touches.retain(|id| !ids.contains(id));
            }
            drop_event(&e);
        });
    }

    pub(super) fn install() {
        let Some(window) = web_sys::window() else { return };
        let flag = window.location().search().ok().and_then(|q| web_sys::UrlSearchParams::new_with_str(&q).ok()).and_then(|p| p.get("stylus"));
        match flag.as_deref() {
            Some("off") => return,
            Some("on") => ENABLED.store(true, Ordering::Relaxed),
            _ => {
                let seen = window.local_storage().ok().flatten().and_then(|s| s.get_item(STORAGE_KEY).ok().flatten());
                ENABLED.store(seen.as_deref() == Some("1"), Ordering::Relaxed);
            }
        }
        let opts = AddEventListenerOptions::new();
        opts.set_capture(true);
        opts.set_passive(false);
        let pointer = Closure::<dyn FnMut(PointerEvent)>::new(on_pointer);
        for ty in ["pointerdown", "pointermove", "pointerup", "pointercancel"] {
            let _ = window.add_event_listener_with_callback_and_add_event_listener_options(ty, pointer.as_ref().unchecked_ref(), &opts);
        }
        let touch = Closure::<dyn FnMut(TouchEvent)>::new(on_touch);
        for ty in ["touchstart", "touchmove", "touchend", "touchcancel"] {
            let _ = window.add_event_listener_with_callback_and_add_event_listener_options(ty, touch.as_ref().unchecked_ref(), &opts);
        }
        // The listeners live as long as the page.
        pointer.forget();
        touch.forget();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comments::CommentTool;

    #[test]
    fn a_finger_pans_with_pen_tools_only_in_stylus_mode() {
        let highlight = QuickTool::Comment(CommentTool::Highlight);
        ENABLED.store(false, Ordering::Relaxed);
        FINGER_PRESS.store(true, Ordering::Relaxed);
        assert_eq!(page_tool(highlight), highlight, "stylus mode off: the finger marks as before");

        ENABLED.store(true, Ordering::Relaxed);
        assert_eq!(page_tool(highlight), QuickTool::Hand, "a finger moves the page");
        assert_eq!(page_tool(QuickTool::Comment(CommentTool::Ink)), QuickTool::Hand);
        assert_eq!(page_tool(QuickTool::Select), QuickTool::Hand);
        assert_eq!(page_tool(QuickTool::AddText), QuickTool::AddText, "tap-to-place tools keep the finger");

        FINGER_PRESS.store(false, Ordering::Relaxed);
        assert_eq!(page_tool(highlight), highlight, "the pen keeps the chosen tool");
        ENABLED.store(false, Ordering::Relaxed);
    }
}
