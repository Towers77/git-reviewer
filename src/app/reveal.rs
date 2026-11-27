//! A list that folds open and closed: a repo's worktrees, a group of changed files. iced
//! 0.14 can't animate a widget's height (a container's height is a fixed size or a
//! fill, never "a share of its content's") or fade it (there's no opacity), so this
//! does both for its content.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell, overlay, renderer};
use iced::{Color, Element, Event, Length, Rectangle, Size, Theme, Vector, mouse};

use super::Message;

/// `content`, `shown` of the way open: 0 is folded away (no height), 1 is all of it,
/// and a little past 1 (the easing's overshoot) is a little taller, with room under
/// it. It's laid out whole and cut to that height, then faded in the same measure by
/// drawing `background` over it, see-through by as much as it's shown: the panel
/// under the list, so that's the same as fading the list itself.
pub(super) fn reveal<'a>(
    content: impl Into<Element<'a, Message>>,
    shown: f32,
    background: Color,
) -> Element<'a, Message> {
    Element::new(Reveal {
        content: content.into(),
        shown: shown.max(0.0),
        background,
    })
}

struct Reveal<'a> {
    content: Element<'a, Message>,
    shown: f32,
    background: Color,
}

impl Widget<Message, Theme, iced::Renderer> for Reveal<'_> {
    // The tree is the content's own; `Reveal` adds no state.
    fn tag(&self) -> widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.content.as_widget().size().width, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        // As tall as the content wants, whatever height is left for this.
        let unbounded =
            layout::Limits::new(limits.min(), Size::new(limits.max().width, f32::INFINITY));
        let content = self
            .content
            .as_widget_mut()
            .layout(tree, renderer, &unbounded);
        let size = content.size();
        layout::Node::with_children(
            Size::new(size.width, size.height * self.shown),
            vec![content],
        )
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let Some(content) = layout.children().next() else {
            return;
        };
        self.content.as_widget_mut().update(
            tree,
            event,
            content,
            visible(cursor, layout.bounds()),
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        use iced::advanced::Renderer as _;

        let bounds = layout.bounds();
        let (Some(content), Some(clip)) = (layout.children().next(), bounds.intersection(viewport))
        else {
            return;
        };

        renderer.with_layer(clip, |renderer| {
            self.content.as_widget().draw(
                tree,
                renderer,
                theme,
                style,
                content,
                visible(cursor, bounds),
                &clip,
            );
        });

        // In a layer of its own, after the content's: within one layer iced draws
        // every background before any text, so the text would show through.
        let fade = 1.0 - self.shown.min(1.0);
        if fade > 0.0 {
            renderer.with_layer(clip, |renderer| {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        ..renderer::Quad::default()
                    },
                    self.background.scale_alpha(fade),
                );
            });
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let Some(content) = layout.children().next() else {
            return mouse::Interaction::None;
        };
        self.content.as_widget().mouse_interaction(
            tree,
            content,
            visible(cursor, layout.bounds()),
            viewport,
            renderer,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some(content) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(tree, content, renderer, operation);
        }
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, iced::Renderer>> {
        let content = layout.children().next()?;
        self.content
            .as_widget_mut()
            .overlay(tree, content, renderer, viewport, translation)
    }
}

/// The cursor, for the content: only where the content is shown, so the part cut off
/// below can't be hovered or clicked through what's drawn there.
fn visible(cursor: mouse::Cursor, bounds: Rectangle) -> mouse::Cursor {
    if cursor.is_over(bounds) {
        cursor
    } else {
        mouse::Cursor::Unavailable
    }
}
