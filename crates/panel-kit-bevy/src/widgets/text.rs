//! Bevy UI port of panel-kit core's text-family content.
//!
//! Core's [`ContentSpec::Text`] carries a body plus a [`ScrollPolicy`], and
//! its [`ContentSpec::Editor`] and [`ContentSpec::Custom`] variants are host
//! bindings: Bevy has no native text editor or arbitrary native view, so this
//! module paints a deterministic placeholder shell and exposes the binding
//! through a marker component for the host to mount into.
//!
//! The policy mapping follows the web painter (`src/widgets/scroll.rs`):
//!
//! - [`ScrollPolicy::Clip`] preserves authored lines with soft wrapping off
//!   and pins the viewport to the parent's height (`height: 100%`,
//!   `min-height: 0`) with overflow hidden.
//! - [`ScrollPolicy::Wrap`] wraps each line at word boundaries (falling back
//!   to characters) and lets the viewport grow with its content.
//! - [`ScrollPolicy::Auto`] asks for the backend's native overflow behavior;
//!   Bevy UI has no native scroll container, so it behaves like `Wrap`.
//!
//! Bodies are split on newline boundaries. Rust's `str::lines` drops the
//! final line terminator, so an empty body paints no line nodes and a body
//! ending in `\n` does not gain a trailing empty line.

use bevy::prelude::*;
use panel_kit_core::widgets::{ContentSpec, DataSource, ScrollPolicy, TextModel};

use crate::theme::BevyTheme;

/// Font size shared by body lines and placeholder labels.
const TEXT_SIZE: f32 = 11.0;

/// Marks the placeholder painted for [`ContentSpec::Custom`].
///
/// A custom binding is application-native content the host paints itself.
/// The node carrying this component is the mount point core's binding names;
/// the visible label keeps an unmapped panel from rendering as a blank body.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct CustomBindingPlaceholder {
    /// Host binding identifier the application resolves.
    pub binding: String,
}

/// Marks the placeholder shell painted for [`ContentSpec::Editor`].
///
/// Bevy has no text editor, so the widget draws the binding and the empty
/// prompt and attaches this component so the host can mount its editor into
/// the same entity. `multiline` is preserved here because the placeholder
/// text cannot express it.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct TextEditorShell {
    /// Host binding identifier the editor reads and writes.
    pub binding: String,
    /// Whether the editor accepts multiple lines.
    pub multiline: bool,
    /// Empty-state prompt text.
    pub placeholder: String,
}

/// Paint one text-family content spec into `parent`.
///
/// [`ContentSpec::Text`] paints its inline body or a `text binding: <id>`
/// placeholder for provider-backed sources. [`ContentSpec::Editor`] and
/// [`ContentSpec::Custom`] paint host placeholders. Every other content kind
/// is left untouched because its own widget module paints it.
///
/// The spawned node is deterministic: the same spec always produces the same
/// entity tree, with authored lines in source order.
pub fn spawn(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, content: &ContentSpec) {
    match content {
        ContentSpec::Text { source, scroll } => spawn_text(parent, theme, source, *scroll),
        ContentSpec::Editor {
            binding,
            multiline,
            placeholder,
        } => spawn_editor_shell(parent, theme, binding, *multiline, placeholder),
        ContentSpec::Custom { binding } => spawn_custom_placeholder(parent, theme, binding),
        _ => {}
    }
}

/// Paint a text body (inline or bound) under the requested scroll policy.
fn spawn_text(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    source: &DataSource<TextModel>,
    scroll: ScrollPolicy,
) {
    let (lines, tint) = text_lines(source, theme);

    parent
        .spawn(viewport_node(scroll))
        .with_children(|viewport| {
            for line in &lines {
                spawn_line(viewport, line, tint, scroll != ScrollPolicy::Clip);
            }
        });
}

/// Authored lines plus the colour that marks whether they are host-resolved.
///
/// An unresolved binding is muted, matching the custom-binding placeholder;
/// authored text uses the primary text colour.
fn text_lines(source: &DataSource<TextModel>, theme: &BevyTheme) -> (Vec<String>, Color) {
    match source {
        DataSource::Inline { value } => (
            value.text.lines().map(str::to_string).collect(),
            theme.text,
        ),
        DataSource::Binding { id } => (vec![format!("text binding: {id}")], theme.text_muted),
    }
}

/// Column viewport that carries the scroll policy.
fn viewport_node(scroll: ScrollPolicy) -> Node {
    let column = column_node();

    match scroll {
        // A clip viewport takes its height from the parent instead of growing
        // with the text, so the hidden overflow is what limits the body.
        ScrollPolicy::Clip => Node {
            height: Val::Percent(100.0),
            min_height: Val::Px(0.0),
            overflow: Overflow::clip(),
            ..column
        },
        ScrollPolicy::Wrap | ScrollPolicy::Auto => Node {
            overflow: Overflow::visible(),
            ..column
        },
    }
}

/// One authored line as a text entity.
///
/// Wrapping policy lives on the line because Bevy applies it per text block:
/// wrapping lines are width-constrained and break at word boundaries, clipped
/// lines keep their authored line breaks and never soft-wrap.
fn spawn_line(parent: &mut ChildSpawnerCommands, value: &str, tint: Color, wraps: bool) {
    let layout = if wraps {
        TextLayout::new_with_linebreak(LineBreak::WordOrCharacter)
    } else {
        TextLayout::new_with_no_wrap()
    };
    let node = if wraps {
        Node {
            width: Val::Percent(100.0),
            ..default()
        }
    } else {
        Node::default()
    };

    parent.spawn((
        Text::new(value),
        node,
        TextFont {
            font_size: TEXT_SIZE,
            ..default()
        },
        TextColor(tint),
        layout,
    ));
}

/// Placeholder shell for a native editor binding.
///
/// The prompt line is painted even when it is empty so the shell keeps the
/// same two-line shape whether or not the spec authored a placeholder.
fn spawn_editor_shell(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    binding: &str,
    multiline: bool,
    placeholder: &str,
) {
    let shell = TextEditorShell {
        binding: binding.to_string(),
        multiline,
        placeholder: placeholder.to_string(),
    };

    parent
        .spawn((shell, column_node()))
        .with_children(|shell| {
            spawn_line(shell, &format!("editor binding: {binding}"), theme.text, true);
            spawn_line(shell, placeholder, theme.text_muted, true);
        });
}

/// Placeholder for application-native content addressed by a binding.
fn spawn_custom_placeholder(parent: &mut ChildSpawnerCommands, theme: &BevyTheme, binding: &str) {
    let placeholder = CustomBindingPlaceholder {
        binding: binding.to_string(),
    };

    parent
        .spawn((placeholder, column_node()))
        .with_children(|shell| {
            spawn_line(shell, &format!("native binding: {binding}"), theme.text_muted, true);
        });
}

/// Full-width column shared by viewports and host placeholder shells.
fn column_node() -> Node {
    Node {
        width: Val::Percent(100.0),
        flex_direction: FlexDirection::Column,
        ..default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Paint one spec under a bare root and run the spawn system to completion.
    fn paint(content: ContentSpec) -> App {
        let mut app = App::new();
        app.add_systems(Startup, move |mut commands: Commands| {
            let theme = BevyTheme::default();
            commands
                .spawn(Node::default())
                .with_children(|parent| spawn(parent, &theme, &content));
        });
        app.update();
        app
    }

    fn root(app: &mut App) -> Entity {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<Entity, (With<Node>, Without<ChildOf>)>();
        roots.single(world).expect("one root node")
    }

    /// The single content node `spawn` paints under the root.
    fn content_node(app: &mut App) -> Entity {
        let root = root(app);
        let children = app
            .world()
            .get::<Children>(root)
            .expect("spawn paints one content node");
        assert_eq!(children.len(), 1, "spawn paints exactly one content node");
        children[0]
    }

    fn child_entities(app: &App, container: Entity) -> Vec<Entity> {
        app.world()
            .get::<Children>(container)
            .map(|children| children.to_vec())
            .unwrap_or_default()
    }

    fn child_texts(app: &App, container: Entity) -> Vec<String> {
        let world = app.world();
        child_entities(app, container)
            .iter()
            .filter_map(|child| world.get::<Text>(*child).map(|text| text.0.clone()))
            .collect()
    }

    fn child_layouts(app: &App, container: Entity) -> Vec<TextLayout> {
        let world = app.world();
        child_entities(app, container)
            .iter()
            .map(|child| *world.get::<TextLayout>(*child).expect("line layout"))
            .collect()
    }

    fn child_colours(app: &App, container: Entity) -> Vec<Color> {
        let world = app.world();
        child_entities(app, container)
            .iter()
            .map(|child| world.get::<TextColor>(*child).expect("line colour").0)
            .collect()
    }

    fn inline_text(body: &str, scroll: ScrollPolicy) -> ContentSpec {
        ContentSpec::Text {
            source: DataSource::Inline {
                value: TextModel {
                    text: body.to_string(),
                },
            },
            scroll,
        }
    }

    fn bound_text(id: &str, scroll: ScrollPolicy) -> ContentSpec {
        ContentSpec::Text {
            source: DataSource::Binding { id: id.to_string() },
            scroll,
        }
    }

    // Ported from core's `data_source_preserves_inline_and_binding_sources`
    // and the TUI canary's `render_text`: inline bodies paint their text,
    // bindings paint an unresolved placeholder in the muted colour.
    #[test]
    fn inline_and_bound_text_sources_paint_distinct_bodies() {
        let mut inline = paint(inline_text("hello", ScrollPolicy::Wrap));
        let inline_node = content_node(&mut inline);
        assert_eq!(child_texts(&inline, inline_node), ["hello"]);
        assert_eq!(child_colours(&inline, inline_node), [BevyTheme::default().text]);

        let mut bound = paint(bound_text("notes.body", ScrollPolicy::Wrap));
        let bound_node = content_node(&mut bound);
        assert_eq!(
            child_texts(&bound, bound_node),
            ["text binding: notes.body"]
        );
        assert_eq!(child_colours(&bound, bound_node), [BevyTheme::default().text_muted]);
    }

    // Ported from the TUI canary's `render_text`: authored text splits into
    // one node per authored line, in source order.
    #[test]
    fn text_lines_split_on_newlines_in_order() {
        let mut app = paint(inline_text("one\ntwo\nthree", ScrollPolicy::Wrap));
        let node = content_node(&mut app);

        assert_eq!(child_texts(&app, node), ["one", "two", "three"]);
    }

    #[test]
    fn blank_lines_survive_the_split() {
        let mut app = paint(inline_text("one\n\ntwo", ScrollPolicy::Wrap));
        let node = content_node(&mut app);

        assert_eq!(child_texts(&app, node), ["one", "", "two"]);
    }

    #[test]
    fn trailing_newline_does_not_add_a_line() {
        let mut app = paint(inline_text("one\ntwo\n", ScrollPolicy::Wrap));
        let node = content_node(&mut app);

        assert_eq!(child_texts(&app, node), ["one", "two"]);
    }

    // Ported from the web snapshot for `ScrollPolicy::Clip`
    // (`src/widgets/tests.rs::scroll_and_spinner_consume_core_policy_and_model_inputs`):
    // clipping keeps authored lines and hides overflow instead of wrapping.
    #[test]
    fn clip_policy_pins_a_fixed_viewport_and_hides_overflow() {
        let mut app = paint(inline_text("one\ntwo", ScrollPolicy::Clip));
        let node = content_node(&mut app);

        let node = app.world().get::<Node>(node).expect("viewport node");
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.height, Val::Percent(100.0));
        assert_eq!(node.min_height, Val::Px(0.0));
        assert_eq!(node.overflow, Overflow::clip());
        assert_eq!(node.flex_direction, FlexDirection::Column);

        let layout_holder = content_node(&mut app);
        let layouts = child_layouts(&app, layout_holder);
        assert_eq!(layouts.len(), 2);
        for layout in layouts {
            assert_eq!(layout.linebreak, LineBreak::NoWrap);
        }
    }

    // Ported from the web painter's `policy_style`: `Wrap` soft-wraps long
    // lines and lets the viewport grow.
    #[test]
    fn wrap_policy_wraps_lines_without_clipping() {
        let mut app = paint(inline_text("alpha beta", ScrollPolicy::Wrap));
        let node = content_node(&mut app);

        let node_ref = app.world().get::<Node>(node).expect("viewport node");
        assert_eq!(node_ref.width, Val::Percent(100.0));
        assert_eq!(node_ref.height, Val::Auto);
        assert_eq!(node_ref.overflow, Overflow::visible());
        assert_eq!(node_ref.flex_direction, FlexDirection::Column);

        let line = child_entities(&app, node)[0];
        let line_node = app.world().get::<Node>(line).expect("line node");
        assert_eq!(line_node.width, Val::Percent(100.0));

        let layout = app.world().get::<TextLayout>(line).expect("line layout");
        assert_eq!(layout.linebreak, LineBreak::WordOrCharacter);
    }

    // `Auto` defers to the renderer's native overflow behavior. Bevy UI has
    // no native scroll container, so it takes the same flow as `Wrap`.
    #[test]
    fn auto_policy_uses_bevy_flow_like_wrap() {
        let mut app = paint(inline_text("auto wrapped", ScrollPolicy::Auto));
        let node = content_node(&mut app);

        let node_ref = app.world().get::<Node>(node).expect("viewport node");
        assert_eq!(node_ref.height, Val::Auto);
        assert_eq!(node_ref.overflow, Overflow::visible());

        let layout = child_layouts(&app, node).remove(0);
        assert_eq!(layout.linebreak, LineBreak::WordOrCharacter);
    }

    #[test]
    fn text_lines_use_the_primary_text_colour() {
        let mut app = paint(inline_text("one\ntwo", ScrollPolicy::Wrap));
        let node = content_node(&mut app);

        assert_eq!(child_colours(&app, node), [BevyTheme::default().text; 2]);
    }

    #[test]
    fn empty_text_paints_an_empty_viewport() {
        let mut app = paint(inline_text("", ScrollPolicy::Wrap));
        let node = content_node(&mut app);

        assert!(child_texts(&app, node).is_empty());
        assert!(child_entities(&app, node).is_empty());

        let node_ref = app.world().get::<Node>(node).expect("viewport node");
        assert_eq!(node_ref.width, Val::Percent(100.0));
        assert_eq!(node_ref.flex_direction, FlexDirection::Column);
    }

    // Ported from the TUI canary's `render_editor_binding` and the web
    // painter's `paint_editor`: the shell names the binding and shows the
    // prompt, and the marker carries the model fields the host needs.
    #[test]
    fn editor_paints_a_binding_and_placeholder_shell() {
        let content = ContentSpec::Editor {
            binding: "editor.body".to_string(),
            multiline: true,
            placeholder: "type here".to_string(),
        };
        let mut app = paint(content);
        let node = content_node(&mut app);

        assert_eq!(
            child_texts(&app, node),
            ["editor binding: editor.body", "type here"]
        );
        assert_eq!(child_colours(&app, node), [BevyTheme::default().text, BevyTheme::default().text_muted]);

        let shell = app
            .world()
            .get::<TextEditorShell>(node)
            .expect("editor shell marker");
        assert_eq!(shell.binding, "editor.body");
        assert!(shell.multiline);
        assert_eq!(shell.placeholder, "type here");
    }

    #[test]
    fn editor_shell_preserves_a_single_line_binding() {
        let content = ContentSpec::Editor {
            binding: "editor.title".to_string(),
            multiline: false,
            placeholder: String::new(),
        };
        let mut app = paint(content);
        let node = content_node(&mut app);

        assert_eq!(
            child_texts(&app, node),
            ["editor binding: editor.title", ""]
        );

        let shell = app
            .world()
            .get::<TextEditorShell>(node)
            .expect("editor shell marker");
        assert!(!shell.multiline);
        assert!(shell.placeholder.is_empty());
    }

    // Ported from the TUI canary's `render_binding_placeholder` and the web
    // painter's `ContentView::Custom` branch: an unmapped binding stays
    // visible and muted.
    #[test]
    fn custom_binding_paints_a_muted_native_placeholder() {
        let mut app = paint(ContentSpec::Custom {
            binding: "procgen.forge".to_string(),
        });
        let node = content_node(&mut app);

        assert_eq!(
            child_texts(&app, node),
            ["native binding: procgen.forge"]
        );
        assert_eq!(child_colours(&app, node), [BevyTheme::default().text_muted]);

        let placeholder = app
            .world()
            .get::<CustomBindingPlaceholder>(node)
            .expect("custom binding marker");
        assert_eq!(placeholder.binding, "procgen.forge");
    }

    // Documented invariant: a text widget paints only the text-family
    // variants; every other kind belongs to its own widget module.
    #[test]
    fn non_text_content_is_left_to_its_own_widget() {
        let mut app = paint(ContentSpec::Spinner {
            label: DataSource::Inline {
                value: "indexing".to_string(),
            },
        });

        let root = root(&mut app);
        let children = app.world().get::<Children>(root);
        assert_eq!(children.map_or(0, |children| children.len()), 0);

        let mut texts = app.world_mut().query::<&Text>();
        assert_eq!(texts.iter(app.world()).count(), 0);
    }
}
