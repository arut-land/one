//! Frame-clock motion for the two things GTK CSS cannot animate in this
//! surface: the sidebar pane's position and the transcript's scroll offset.
//! CSS transitions and keyframes handle everything that is a style; this is
//! for values a widget owns. One curve, one duration policy, and the desktop's
//! reduced-motion answer honoured here rather than at every call site.
use gtk::{glib, prelude::*};
use std::{cell::Cell, rc::Rc};

/// libadwaita's default timed-animation duration, so pane and scroll motion
/// keep step with the CSS transitions written to the same figure.
pub const DURATION_MS: u32 = 250;

/// Whether the desktop asked for motion at all. `gtk-enable-animations` off
/// is the older way of asking for reduced motion; GTK 4.22's own setting is
/// the newer one, and either answer wins.
pub fn enabled(widget: &impl IsA<gtk::Widget>) -> bool {
    let settings = gtk::Settings::for_display(&WidgetExt::display(widget));
    settings.is_gtk_enable_animations()
        && settings.gtk_interface_reduced_motion() != gtk::ReducedMotion::Reduce
}

/// Ease-out cubic: fast to leave, settles gently, the curve a pane or a scroll
/// is expected to follow.
fn ease(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

/// A running animation. Dropping it stops the ticks, which is how a newer
/// motion on the same value cancels an older one.
pub struct Motion {
    id: Option<gtk::TickCallbackId>,
    done: Rc<Cell<bool>>,
}

impl Drop for Motion {
    fn drop(&mut self) {
        if !self.done.get()
            && let Some(id) = self.id.take()
        {
            id.remove();
        }
    }
}

/// Drives `step` from 0.0 to 1.0 over `duration_ms` on `widget`'s frame clock.
/// When motion is off, or the widget has no frame clock to tick on, `step`
/// receives 1.0 at once and nothing is returned: the value still lands.
pub fn run(
    widget: &impl IsA<gtk::Widget>,
    duration_ms: u32,
    step: impl Fn(f64) + 'static,
) -> Option<Motion> {
    if duration_ms == 0 || !widget.is_mapped() || !enabled(widget) {
        step(1.0);
        return None;
    }
    let done = Rc::new(Cell::new(false));
    let began = Cell::new(None::<i64>);
    let duration = f64::from(duration_ms) * 1000.0;
    let id = widget.add_tick_callback({
        let done = done.clone();
        move |_, clock| {
            let now = clock.frame_time();
            let start = began.get().unwrap_or_else(|| {
                began.set(Some(now));
                now
            });
            let progress = ((now - start) as f64 / duration).clamp(0.0, 1.0);
            step(ease(progress));
            if progress >= 1.0 {
                done.set(true);
                glib::ControlFlow::Break
            } else {
                glib::ControlFlow::Continue
            }
        }
    });
    Some(Motion { id: Some(id), done })
}
