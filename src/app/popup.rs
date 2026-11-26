//! A menu that opens under a widget, over everything else, like VS Code's dropdowns: the
//! sidebar's repo menu. iced 0.14 has no such widget: `float` lays its content out where
//! it is (it would take up room in the row) and `tooltip` only follows the hover, so this
//! anchors an overlay of its own.

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell, overlay, renderer};
use iced::keyboard::{self, Key, key::Named};
use iced::{Element, Event, Length, Point, Rectangle, Size, Theme, Vector, mouse};

use super::Message;

/// The space between the anchor and the menu.
const GAP: f32 = 4.0;

/// `anchor`, with `menu` open under it while it's `Some`: right-aligned with the anchor,
/// or above it if there's no room below. While it's open, nothing else reacts to the
/// mouse or the keyboard: a click outside it, or Escape, only sends `on_dismiss`, like a
/// click outside a VS Code menu closes it without clicking what's under it.
pub(super) fn popup<'a>(
    anchor: impl Into<Element<'a, Message>>,
    menu: Option<Element<'a, Message>>,
    on_dismiss: Message,
) -> Element<'a, Message> {
    Element::new(Popup {
        anchor: anchor.into(),
        menu,
        on_dismiss,
    })
}

struct Popup<'a> {
    anchor: Element<'a, Message>,
    menu: Option<Element<'a, Message>>,
    on_dismiss: Message,
}

// The anchor is laid out, drawn and clicked as if `Popup` weren't there; the menu only
// exists as the overlay. The tree holds the anchor's state, then the menu's while open.
impl Widget<Message, Theme, iced::Renderer> for Popup<'_> {
    fn children(&self) -> Vec<Tree> {
        let mut children = vec![Tree::new(&self.anchor)];
        if let Some(menu) = &self.menu {
            children.push(Tree::new(menu));
        }
        children
    }

    fn diff(&self, tree: &mut Tree) {
        match &self.menu {
            Some(menu) => tree.diff_children(&[&self.anchor, menu]),
            None => tree.diff_children(&[&self.anchor]),
        }
    }

    fn size(&self) -> Size<Length> {
        self.anchor.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.anchor.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.anchor
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
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
        self.anchor.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
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
        self.anchor.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.anchor.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
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
        self.anchor
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, iced::Renderer>> {
        let Some(menu) = &mut self.menu else {
            return self.anchor.as_widget_mut().overlay(
                &mut tree.children[0],
                layout,
                renderer,
                viewport,
                translation,
            );
        };

        Some(overlay::Element::new(Box::new(Menu {
            content: menu,
            tree: &mut tree.children[1],
            // In window coordinates: inside a scrollable, `translation` is its scroll.
            anchor: layout.bounds() + translation,
            on_dismiss: self.on_dismiss.clone(),
        })))
    }
}

/// The open menu. Its layout covers the whole window, with the menu as its one child:
/// iced draws an overlay clipped to its layout, which would cut off the menu's shadow,
/// and covering the window is how it keeps the mouse from everything under it.
struct Menu<'a, 'b> {
    content: &'a mut Element<'b, Message>,
    tree: &'a mut Tree,
    anchor: Rectangle,
    on_dismiss: Message,
}

impl overlay::Overlay<Message, Theme, iced::Renderer> for Menu<'_, '_> {
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::ZERO, bounds);
        let menu = self
            .content
            .as_widget_mut()
            .layout(self.tree, renderer, &limits);
        let size = menu.size();

        let below = self.anchor.y + self.anchor.height + GAP;
        let y = if below + size.height <= bounds.height {
            below
        } else {
            (self.anchor.y - GAP - size.height).max(0.0)
        };
        let x = (self.anchor.x + self.anchor.width - size.width)
            .min(bounds.width - size.width)
            .max(0.0);

        layout::Node::with_children(bounds, vec![menu.move_to(Point::new(x, y))])
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let Some(menu) = layout.children().next() else {
            return;
        };
        self.content.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            menu,
            cursor,
            &layout.bounds(),
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let Some(menu) = layout.children().next() else {
            return;
        };
        self.content.as_widget_mut().update(
            self.tree,
            event,
            menu,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );
        if shell.is_event_captured() {
            return;
        }

        let dismiss = match event {
            Event::Mouse(mouse::Event::ButtonPressed(_)) => !cursor.is_over(menu.bounds()),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Escape),
                ..
            }) => true,
            _ => false,
        };
        if dismiss {
            shell.publish(self.on_dismiss.clone());
        }
        // Clicks and key presses stop here, so the widgets under the menu never see
        // them: not the row under an outside click, nor the commit box under a key.
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(_) | mouse::Event::ButtonReleased(_))
                | Event::Keyboard(_)
        ) {
            shell.capture_event();
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let Some(menu) = layout.children().next() else {
            return mouse::Interaction::None;
        };
        let interaction = self.content.as_widget().mouse_interaction(
            self.tree,
            menu,
            cursor,
            &layout.bounds(),
            renderer,
        );
        // Anything but `None`, anywhere in the window: iced then gives the widgets
        // under the menu no cursor, so nothing there shows a hover while it's open.
        interaction.max(mouse::Interaction::Idle)
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        if let Some(menu) = layout.children().next() {
            self.content
                .as_widget_mut()
                .operate(self.tree, menu, renderer, operation);
        }
    }
}
