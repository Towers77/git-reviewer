//! The error toasts in the bottom-right corner, like VS Code's notifications: each
//! stays until its × is clicked, newer ones stack below the older ones, and they rise
//! and fade in, then sink and fade out while the stack closes the gap.

use std::time::Instant;

use iced::widget::scrollable::Direction;
use iced::widget::{container, keyed_column, opaque, row, scrollable, text};
use iced::{Animation, Element, Fill, Padding, Vector};

use super::slide::slide;
use super::{Message, changes};
use crate::theme::icons::{self, icon};
use crate::theme::{self, Colors, icon_sizes, layouts, motion, text_sizes};

/// At most this many toasts are shown: a newer one past it sends the oldest away, so a
/// burst of errors can't fill the window.
const MOST_SHOWN: usize = 3;
/// How far a toast rises as it comes in, and sinks as it leaves: back the way it came.
const SHIFT: f32 = 12.0;

#[derive(Default)]
pub(super) struct Toasts {
    /// Oldest first. A closed toast stays here until it's done leaving.
    list: Vec<Toast>,
    next_id: u64,
}

struct Toast {
    /// What the stack is keyed by, so a toast leaving from the middle doesn't hand its
    /// place's widget state (the message's scroll, the ×'s hover) to the one after it.
    id: u64,
    message: String,
    /// `true` while it's shown: it animates to `false` when it's closed.
    shown: Animation<bool>,
}

impl Toasts {
    /// Shows `message` in a new toast, unless an open one already says the same thing.
    pub(super) fn push(&mut self, message: String, now: Instant) {
        if self.open().any(|toast| toast.message == message) {
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.list.push(Toast {
            id,
            message,
            shown: Animation::new(false)
                .duration(motion::DURATION)
                .easing(motion::EASING)
                .go(true, now),
        });

        let extra = self.open().count().saturating_sub(MOST_SHOWN);
        let oldest: Vec<u64> = self.open().take(extra).map(|toast| toast.id).collect();
        for id in oldest {
            self.dismiss(id, now);
        }
    }

    /// Closes toast `id`: it starts leaving.
    pub(super) fn dismiss(&mut self, id: u64, now: Instant) {
        if let Some(toast) = self.list.iter_mut().find(|toast| toast.id == id) {
            toast.shown.go_mut(false, now);
        }
    }

    /// Lets go of the toasts that are done leaving.
    pub(super) fn prune(&mut self, now: Instant) {
        self.list
            .retain(|toast| toast.shown.value() || toast.shown.is_animating(now));
    }

    /// A toast is coming or going: the app asks for frames until it's done.
    pub(super) fn is_animating(&self, now: Instant) -> bool {
        self.list.iter().any(|toast| toast.shown.is_animating(now))
    }

    fn open(&self) -> impl Iterator<Item = &Toast> {
        self.list.iter().filter(|toast| toast.shown.value())
    }

    /// The stack, drawn at `now`, or `None` with no toast to draw.
    pub(super) fn view(&self, c: &Colors, now: Instant) -> Option<Element<'_, Message>> {
        if self.list.is_empty() {
            return None;
        }

        let stack = keyed_column(self.list.iter().map(|toast| {
            // 0 gone, 1 shown, a little past 1 at the overshoot (or below 0 leaving).
            let shown = toast.shown.interpolate(0.0, 1.0, now);
            let opacity = shown.clamp(0.0, 1.0);
            // Below its place by `SHIFT` when gone: it rises from there and sinks back.
            let offset = Vector::new(0.0, SHIFT * (1.0 - shown));
            // The gap above each card is part of its place, so it closes with it.
            let place = container(card(&c.faded(opacity), opacity, toast))
                .padding(Padding::ZERO.top(layouts::GAP));
            // `opaque`: in a stack, the mouse passes through to the layer below wherever
            // the top layer has nothing interactive, so a click on a message's text would
            // land on the row under it. It wraps the place rather than the card, so it
            // still blocks while the card moves (`slide` hides the cursor from it then).
            (toast.id, opaque(slide(place, shown, offset)))
        }));

        // Outside the toasts, clicks go through to the window below, on purpose.
        Some(
            container(stack)
                .align_right(Fill)
                .align_bottom(Fill)
                .padding(layouts::PANEL_PADDING_X)
                .into(),
        )
    }
}

/// One toast's card, in colors faded to `opacity` (see `Colors::faded`). A long
/// message, like a hook's output, scrolls inside it.
fn card<'a>(c: &Colors, opacity: f32, toast: &'a Toast) -> Element<'a, Message> {
    let message = scrollable(
        container(text(&toast.message).size(text_sizes::SMALL).width(Fill))
            // Room for the scrollbar, which is drawn over the content.
            .padding(Padding::ZERO.right(10.0)),
    )
    .direction(Direction::Vertical(theme::thin_scrollbar()))
    .style(theme::scrollbar(c))
    .width(Fill);

    container(
        row![
            // 2px down: centers the icon on the message's first line.
            container(icon(icons::CIRCLE_ALERT, icon_sizes::BUTTON, c.danger))
                .padding(Padding::ZERO.top(2.0)),
            message,
            changes::icon_button(c, icons::X, Message::DismissToast(toast.id), false),
        ]
        .spacing(layouts::GAP),
    )
    .padding([10.0, 12.0])
    .width(layouts::TOAST_WIDTH)
    .max_height(layouts::TOAST_MAX_HEIGHT)
    .style(theme::toast(c, opacity))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(toasts: &Toasts) -> Vec<&str> {
        toasts.open().map(|toast| toast.message.as_str()).collect()
    }

    #[test]
    fn the_same_message_twice_is_one_toast() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push("push failed".into(), now);
        toasts.push("push failed".into(), now);
        assert_eq!(messages(&toasts), ["push failed"]);
    }

    #[test]
    fn a_closed_message_can_come_back() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push("push failed".into(), now);
        toasts.dismiss(0, now);
        toasts.push("push failed".into(), now);
        assert_eq!(messages(&toasts), ["push failed"]);
        assert_eq!(toasts.list.len(), 2, "the closed one is still leaving");
    }

    #[test]
    fn past_the_most_shown_the_oldest_leaves() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        for message in ["a", "b", "c", "d"] {
            toasts.push(message.into(), now);
        }
        assert_eq!(messages(&toasts), ["b", "c", "d"]);
    }

    #[test]
    fn a_toast_is_let_go_once_it_is_done_leaving() {
        let now = Instant::now();
        let mut toasts = Toasts::default();
        toasts.push("a".into(), now);
        toasts.push("b".into(), now);
        toasts.dismiss(0, now);

        toasts.prune(now + motion::DURATION / 2);
        assert_eq!(toasts.list.len(), 2, "still leaving");
        toasts.prune(now + motion::DURATION * 2);
        assert_eq!(toasts.list.iter().map(|t| t.id).collect::<Vec<_>>(), [1]);
        assert!(!toasts.is_animating(now + motion::DURATION * 2));
    }
}
