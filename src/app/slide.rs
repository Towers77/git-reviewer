//! A toast's place in the stack: it grows as the toast comes in and shrinks to nothing
//! as it leaves, so the stack closes the gap, while the toast itself is drawn moved by
//! an offset (rising in, sinking out). `reveal` is the same idea for the lists, but it
//! always clips its content, and a toast's shadow must spill outside its card.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Theme, Vector, mouse};

use super::Message;

/// `content`, laid out whole and taking `shown` of its height (0 none, 1 all of it,
/// a little past 1 while the easing overshoots), drawn moved by `offset`.
pub(super) fn slide<'a>(
    content: impl Into<Element<'a, Message>>,
    shown: f32,
    offset: Vector,
) -> Element<'a, Message> {
    Element::new(Slide {
        content: content.into(),
        shown: shown.max(0.0),
        offset,
    })
}

struct Slide<'a> {
    content: Element<'a, Message>,
    shown: f32,
    offset: Vector,
}

impl Slide<'_> {
    /// The cursor, for the content: none while it's moving. Its layout isn't moved
    /// with it, so the cursor would land on the wrong spot, and a moving toast is
    /// coming or going anyway.
    fn cursor(&self, cursor: mouse::Cursor) -> mouse::Cursor {
        if self.offset == Vector::ZERO {
            cursor
        } else {
            mouse::Cursor::Unavailable
        }
    }
}

impl Widget<Message, Theme, iced::Renderer> for Slide<'_> {
    // The tree is the content's own; `Slide` adds no state.
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
        let cursor = self.cursor(cursor);
        self.content.as_widget_mut().update(
            tree, event, content, cursor, renderer, clipboard, shell, viewport,
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

        let Some(content) = layout.children().next() else {
            return;
        };
        let draw = |renderer: &mut iced::Renderer| {
            renderer.with_translation(self.offset, |renderer| {
                self.content.as_widget().draw(
                    tree,
                    renderer,
                    theme,
                    style,
                    content,
                    self.cursor(cursor),
                    viewport,
                );
            });
        };

        // While the place is shorter than the content, the content is cut to its top
        // and bottom (so a toast coming in shows from the top down, and one leaving
        // doesn't hang over the toast below), but not its sides, where the shadow
        // spills. Settled, nothing is cut, so the whole shadow shows.
        let bounds = layout.bounds();
        if bounds.height < content.bounds().height {
            let clip = Rectangle {
                x: viewport.x,
                width: viewport.width,
                ..bounds
            };
            renderer.with_layer(clip, draw);
        } else {
            draw(renderer);
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
            self.cursor(cursor),
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
