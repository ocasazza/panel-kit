//! Bevy UI port of panel-kit core's `scroll` component.
//!
//! Core owns the scrolling policy: [`ContentScroll`] measures one body's
//! content and viewport extents in cells, clamps both offsets, and resolves the
//! wheel precedence every backend shares — the content consumes a gesture until
//! its edge, then the gesture bubbles to the workspace. This module wraps that
//! policy in [`ScrollState`], the Bevy component and resource a viewport
//! carries, and projects it onto `bevy_ui`: [`spawn`] builds a clipped node
//! whose [`ScrollPosition`] Bevy's layout applies to the content.
//!
//! Cells keep their core meaning: one cell is one text line vertically and one
//! character advance horizontally. [`ScrollState::offset_px`] converts them
//! with the state's line height and cell width, so one set of offsets drives
//! both the core policy and the pixel-space Bevy rendering.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::text::LineHeight;
use bevy::ui::RelativeCursorPosition;
use panel_kit_core::reducer::WheelDisposition;
use panel_kit_core::widgets::scroll::ContentScroll;

use crate::theme::BevyTheme;

/// Default height of one content line in logical pixels.
pub const LINE_HEIGHT: f32 = 16.0;

/// Default width of one character cell in logical pixels.
pub const CELL_WIDTH: f32 = 8.0;

/// Marks a scroll container's clipped viewport node.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrollViewport;

/// Marks the single content node a scroll viewport offsets.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrollContent;

/// One panel body's scroll offsets in cells plus the extents that bound them.
///
/// A copy of core's [`ContentScroll`] policy with the measured viewport kept
/// alongside it: the viewport supplies the page-step size, which core's state
/// does not record. Insert it as a component on a viewport — as [`spawn`]
/// does — or as a resource for a host with a single scroll region.
#[derive(Component, Resource, Clone, Copy, Debug, PartialEq)]
pub struct ScrollState {
    scroll: ContentScroll,
    viewport: (u16, u16),
    line_height: f32,
    cell_width: f32,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self::new()
    }
}

impl ScrollState {
    /// Create a state at the top-left origin with no measured overflow.
    pub fn new() -> Self {
        Self::with_metrics(LINE_HEIGHT, CELL_WIDTH)
    }

    /// Create a state with explicit rendering metrics.
    ///
    /// `line_height` and `cell_width` are logical pixels per cell; both are
    /// clamped to at least one pixel so pixel-to-cell conversion stays finite.
    pub fn with_metrics(line_height: f32, cell_width: f32) -> Self {
        Self {
            scroll: ContentScroll::new(),
            viewport: (0, 0),
            line_height: line_height.max(1.0),
            cell_width: cell_width.max(1.0),
        }
    }

    /// A state whose line height is the theme's body line box
    /// (`--body-size` × `--body-line-height`), the `.pk-scroll-text` line.
    pub fn for_theme(theme: &BevyTheme) -> Self {
        Self::with_metrics(
            theme.metrics.body_size * theme.metrics.line_height,
            CELL_WIDTH,
        )
    }

    /// Current `(x, y)` offsets in cells.
    pub fn offset(&self) -> (u16, u16) {
        self.scroll.offset()
    }

    /// Current `(x, y)` scrollable maxima in cells.
    pub fn max(&self) -> (u16, u16) {
        self.scroll.max()
    }

    /// Last measured `(width, height)` viewport in cells.
    pub fn viewport(&self) -> (u16, u16) {
        self.viewport
    }

    /// Logical pixels per content line.
    pub fn line_height(&self) -> f32 {
        self.line_height
    }

    /// Logical pixels per character cell.
    pub fn cell_width(&self) -> f32 {
        self.cell_width
    }

    /// Whether the measured content overflows its viewport on either axis.
    pub fn is_scrollable(&self) -> bool {
        self.scroll.is_scrollable()
    }

    /// Record the content and viewport extents in cells, recompute the maxima,
    /// and clamp the offsets into them.
    ///
    /// Returns whether clamping moved either offset, which happens when a panel
    /// shrank since the previous measurement.
    pub fn set_extent(
        &mut self,
        content_w: u16,
        content_h: u16,
        view_w: u16,
        view_h: u16,
    ) -> bool {
        self.viewport = (view_w, view_h);
        self.scroll.set_extent(content_w, content_h, view_w, view_h)
    }

    /// Measure text lines against a viewport in cells and record the extents.
    ///
    /// Content width is the widest line in characters and height is the line
    /// count — the same measurement the TUI text painter records.
    pub fn measure_lines<S: AsRef<str>>(
        &mut self,
        lines: &[S],
        view_w: u16,
        view_h: u16,
    ) -> bool {
        let content_w = lines.iter().map(|line| line.as_ref().chars().count()).max();
        let content_h = lines.len();
        self.set_extent(cells(content_w.unwrap_or(0)), cells(content_h), view_w, view_h)
    }

    /// Apply integer line and column steps from keyboard or page scrolling.
    ///
    /// Returns whether either offset moved.
    pub fn scroll_by(&mut self, delta_x: i32, delta_y: i32) -> bool {
        self.scroll.scroll_by(delta_x, delta_y)
    }

    /// Scroll vertically by whole lines, positive toward the bottom.
    pub fn scroll_lines(&mut self, lines: i32) -> bool {
        self.scroll_by(0, lines)
    }

    /// Lines moved by one page step: one viewport height.
    pub fn page_lines(&self) -> i32 {
        i32::from(self.viewport.1)
    }

    /// Scroll by whole viewports, positive toward the bottom.
    pub fn scroll_pages(&mut self, pages: i32) -> bool {
        self.scroll_by(0, pages.saturating_mul(self.page_lines()))
    }

    /// Apply wheel deltas in cells (positive is right/down) and report whether
    /// the content consumed them.
    ///
    /// Non-finite deltas are ignored. A gesture that cannot move either axis
    /// returns [`WheelDisposition::BubbleToWorkspace`] — the content edge, or a
    /// body whose content fits.
    pub fn absorb_wheel(&mut self, delta_x: f64, delta_y: f64) -> WheelDisposition {
        self.scroll.absorb_wheel(delta_x, delta_y)
    }

    /// Apply one Bevy wheel gesture, translating its unit into cells.
    ///
    /// [`MouseScrollUnit::Line`] already counts lines and columns;
    /// [`MouseScrollUnit::Pixel`] is divided by the state's metrics. This is
    /// the pure entry point for hosts that translate input themselves;
    /// [`wheel_scroll_system`] is the ECS shell over it.
    pub fn apply(&mut self, wheel: &MouseWheel) -> WheelDisposition {
        let (delta_x, delta_y) = self.wheel_cells(wheel.unit, wheel.x, wheel.y);
        self.absorb_wheel(delta_x, delta_y)
    }

    /// Return to the top-left origin, keeping the measured maxima.
    pub fn reset(&mut self) {
        self.scroll.reset();
    }

    /// The offset Bevy's [`ScrollPosition`] takes, in logical pixels.
    pub fn offset_px(&self) -> Vec2 {
        let (x, y) = self.offset();
        Vec2::new(
            f32::from(x) * self.cell_width,
            f32::from(y) * self.line_height,
        )
    }

    fn wheel_cells(&self, unit: MouseScrollUnit, x: f32, y: f32) -> (f64, f64) {
        match unit {
            MouseScrollUnit::Line => (f64::from(x), f64::from(y)),
            MouseScrollUnit::Pixel => (
                f64::from(x) / f64::from(self.cell_width),
                f64::from(y) / f64::from(self.line_height),
            ),
        }
    }
}

/// Saturate a measured cell count into core's `u16` extent domain.
fn cells(count: usize) -> u16 {
    u16::try_from(count).unwrap_or(u16::MAX)
}

/// Spawn a clipped scroll viewport holding `lines`.
///
/// The returned entity is the viewport: it carries [`ScrollViewport`],
/// [`ScrollState`], the [`ScrollPosition`] projected from `state`, and a
/// [`RelativeCursorPosition`] so [`wheel_scroll_system`] can route the wheel to
/// it. Its single [`ScrollContent`] child holds one text node per line.
///
/// The viewport fills its parent and clips both axes, because core tracks
/// horizontal and vertical offsets. The caller measures the cell extents with
/// [`ScrollState::set_extent`] or [`ScrollState::measure_lines`] before
/// spawning; `spawn` only projects the state it is given.
pub fn spawn<S: Into<String>>(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    state: &ScrollState,
    lines: impl IntoIterator<Item = S>,
) -> Entity {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll(),
                ..default()
            },
            ScrollViewport,
            *state,
            ScrollPosition::from(state.offset_px()),
            RelativeCursorPosition::default(),
        ))
        .with_children(|viewport| {
            spawn_content(viewport, theme, state.line_height(), lines)
        })
        .id()
}

/// Spawn the content column under a viewport.
fn spawn_content<S: Into<String>>(
    parent: &mut ChildSpawnerCommands,
    theme: &BevyTheme,
    line_height: f32,
    lines: impl IntoIterator<Item = S>,
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            ScrollContent,
        ))
        .with_children(|content| {
            for line in lines {
                // An empty line keeps its line box, as a `pre` line does.
                content.spawn((
                    Node {
                        min_height: Val::Px(line_height),
                        ..default()
                    },
                    Text::new(line),
                    TextFont {
                        font_size: theme.metrics.body_size,
                        line_height: LineHeight::Px(line_height),
                        ..default()
                    },
                    TextColor(theme.text),
                    TextLayout::new_with_no_wrap(),
                ));
            }
        });
}

/// Applies mouse-wheel input to every scroll viewport under the cursor.
///
/// Each hovered viewport resolves the gesture through core's policy: it moves
/// while either axis still has room, and a gesture the content cannot consume
/// is left for the host's workspace scrolling instead of trapping the wheel.
/// Pair it with [`sync_scroll_position_system`] — [`ScrollPlugin`] chains both —
/// so Bevy's layout sees the new offset.
pub fn wheel_scroll_system(
    mut wheels: EventReader<MouseWheel>,
    mut viewports: Query<(&RelativeCursorPosition, &mut ScrollState)>,
) {
    for wheel in wheels.read() {
        for (cursor, mut state) in &mut viewports {
            if cursor.mouse_over() {
                state.apply(wheel);
            }
        }
    }
}

/// Copies each changed [`ScrollState`] offset into Bevy's [`ScrollPosition`].
///
/// Run it after anything that changes a state: keyboard steps, host code, or
/// [`wheel_scroll_system`]. [`ScrollPlugin`] registers the pair in order.
pub fn sync_scroll_position_system(
    mut viewports: Query<(&ScrollState, &mut ScrollPosition), Changed<ScrollState>>,
) {
    for (state, mut position) in &mut viewports {
        let offset = state.offset_px();
        position.offset_x = offset.x;
        position.offset_y = offset.y;
    }
}

/// Records each viewport's laid-out size and its content's size, in cells,
/// as the state's extents, so the wheel is bounded by what Bevy's layout
/// actually produced. A state only changes when the measurement does.
pub fn measure_scroll_extent_system(
    mut viewports: Query<(&ComputedNode, &Children, &mut ScrollState), With<ScrollViewport>>,
    contents: Query<&ComputedNode, With<ScrollContent>>,
) {
    for (view_node, children, mut state) in &mut viewports {
        let Some(content_node) = children.iter().find_map(|child| contents.get(child).ok()) else {
            continue;
        };
        let logical = |node: &ComputedNode| node.size() * node.inverse_scale_factor();
        let (view, content) = (logical(view_node), logical(content_node));
        let (cell_w, line_h) = (state.cell_width(), state.line_height());
        let mut next = *state;
        next.set_extent(
            cells((content.x / cell_w).ceil() as usize),
            cells((content.y / line_h).ceil() as usize),
            cells((view.x / cell_w).floor() as usize),
            cells((view.y / line_h).floor() as usize),
        );
        if next != *state {
            *state = next;
        }
    }
}

/// Registers layout measurement, the mouse wheel, and the state-to-layout
/// projection.
///
/// Add it when the host uses [`spawn`] viewports. Event registration is
/// idempotent and composes with the workspace's input plugin.
pub struct ScrollPlugin;

impl Plugin for ScrollPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MouseWheel>().add_systems(
            Update,
            (
                measure_scroll_extent_system,
                wheel_scroll_system,
                sync_scroll_position_system,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A state measured like the core scroll tests: extents in cells.
    fn scrolling(content_w: u16, content_h: u16, view_w: u16, view_h: u16) -> ScrollState {
        let mut state = ScrollState::new();
        state.set_extent(content_w, content_h, view_w, view_h);
        state
    }

    fn wheel(unit: MouseScrollUnit, x: f32, y: f32) -> MouseWheel {
        MouseWheel {
            unit,
            x,
            y,
            window: Entity::PLACEHOLDER,
        }
    }

    fn hovered() -> RelativeCursorPosition {
        RelativeCursorPosition {
            normalized_visible_node_rect: Rect::new(0.0, 0.0, 1.0, 1.0),
            normalized: Some(Vec2::splat(0.5)),
        }
    }

    #[test]
    fn maxima_follow_core_saturating_math() {
        assert_eq!(scrolling(10, 10, 10, 4).max(), (0, 6));
        assert_eq!(scrolling(4, 4, 4, 4).max(), (0, 0));
        assert_eq!(scrolling(2, 2, 4, 4).max(), (0, 0));
    }

    #[test]
    fn measuring_lines_records_the_painter_extent() {
        let lines = [
            "row-0", "row-1", "row-2", "row-3", "row-4", "row-5", "row-6", "row-7", "row-8",
            "row-9",
        ];
        let mut state = ScrollState::new();
        assert!(!state.measure_lines(&lines, 11, 4));
        // Six characters in an 11-cell body; ten rows in a four-row body.
        assert_eq!(state.max(), (0, 6));
        assert_eq!(state.viewport(), (11, 4));
        assert!(state.is_scrollable());

        let mut fitting = ScrollState::new();
        assert!(!fitting.measure_lines(&lines[..3], 11, 4));
        assert!(!fitting.is_scrollable());
    }

    #[test]
    fn set_extent_computes_maxima_and_reports_clamping() {
        let mut state = ScrollState::new();
        assert!(!state.set_extent(40, 20, 10, 6));
        assert_eq!(state.max(), (30, 14));
        assert!(state.is_scrollable());

        assert_eq!(
            state.absorb_wheel(0.0, 14.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset(), (0, 14));

        assert!(state.set_extent(40, 8, 10, 6));
        assert_eq!(state.offset(), (0, 2));
        assert_eq!(state.max(), (30, 2));
    }

    #[test]
    fn content_that_fits_bubbles_in_every_direction() {
        let mut state = scrolling(6, 3, 10, 6);
        assert!(!state.is_scrollable());
        for (delta_x, delta_y) in [(0.0, 3.0), (0.0, -3.0), (3.0, 0.0), (-3.0, 0.0)] {
            assert_eq!(
                state.absorb_wheel(delta_x, delta_y),
                WheelDisposition::BubbleToWorkspace
            );
        }
        assert_eq!(state.offset(), (0, 0));
    }

    #[test]
    fn the_bottom_edge_consumes_to_the_maximum_then_bubbles() {
        let mut state = scrolling(12, 10, 4, 4);
        assert_eq!(state.max(), (8, 6));

        assert_eq!(
            state.absorb_wheel(0.0, 3.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset().1, 3);
        assert_eq!(
            state.absorb_wheel(0.0, 6.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset().1, 6);
        assert_eq!(
            state.absorb_wheel(0.0, 1.0),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(state.offset().1, 6);

        assert_eq!(
            state.absorb_wheel(0.0, -2.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset().1, 4);
        assert_eq!(
            state.absorb_wheel(0.0, -99.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset().1, 0);
        assert_eq!(
            state.absorb_wheel(0.0, -1.0),
            WheelDisposition::BubbleToWorkspace
        );
    }

    #[test]
    fn the_right_and_left_edges_consume_to_the_maximum_then_bubble() {
        let mut state = scrolling(12, 10, 4, 4);

        assert_eq!(
            state.absorb_wheel(2.0, 0.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset().0, 2);
        assert_eq!(
            state.absorb_wheel(99.0, 0.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset().0, 8);
        assert_eq!(
            state.absorb_wheel(1.0, 0.0),
            WheelDisposition::BubbleToWorkspace
        );

        assert_eq!(
            state.absorb_wheel(-99.0, 0.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset().0, 0);
        assert_eq!(
            state.absorb_wheel(-1.0, 0.0),
            WheelDisposition::BubbleToWorkspace
        );
    }

    #[test]
    fn a_diagonal_wheel_consumes_when_either_axis_has_room() {
        let mut state = scrolling(12, 10, 4, 4);
        assert_eq!(
            state.absorb_wheel(0.0, -1.0),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            state.absorb_wheel(0.0, 2.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(
            state.absorb_wheel(-1.0, -1.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset(), (0, 1));

        let mut at_left_edge = scrolling(12, 10, 12, 4);
        assert_eq!(at_left_edge.max().0, 0);
        assert_eq!(
            at_left_edge.absorb_wheel(-1.0, 1.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(at_left_edge.offset(), (0, 1));
    }

    #[test]
    fn fractional_and_non_finite_deltas_clamp_without_panicking() {
        let mut state = scrolling(12, 10, 4, 4);
        assert_eq!(
            state.absorb_wheel(0.0, f64::NAN),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            state.absorb_wheel(f64::INFINITY, 0.0),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            state.absorb_wheel(0.0, -f64::INFINITY),
            WheelDisposition::BubbleToWorkspace
        );
        assert_eq!(
            state.absorb_wheel(99.0, -99.0),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset(), (8, 0));
        assert_eq!(
            state.absorb_wheel(1.2, 1.2),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset(), (8, 1));
    }

    #[test]
    fn line_steps_share_the_wheel_clamping() {
        let mut state = scrolling(12, 10, 4, 4);
        assert!(state.scroll_by(0, 20));
        assert_eq!(state.offset().1, 6);
        assert!(!state.scroll_by(0, 20));
        assert!(!state.scroll_by(-99, 0));
        assert!(state.scroll_by(6, -6));
        assert_eq!(state.offset(), (6, 0));
        assert!(state.scroll_lines(1));
        assert_eq!(state.offset(), (6, 1));
        assert!(state.scroll_lines(-99));
        assert_eq!(state.offset(), (6, 0));
        assert!(state.scroll_by(-99, 0));
        assert_eq!(state.offset(), (0, 0));
        assert!(state.scroll_lines(6));
        assert!(state.scroll_by(99, 0));
        assert_eq!(state.offset(), (8, 6));
        assert!(!state.scroll_lines(1));

        state.scroll_by(4, 4);
        state.reset();
        assert_eq!(state.offset(), (0, 0));
        assert_eq!(state.max(), (8, 6));
    }

    #[test]
    fn page_steps_move_one_viewport_and_clamp() {
        let mut state = scrolling(20, 40, 20, 6);
        assert_eq!(state.max(), (0, 34));
        assert_eq!(state.page_lines(), 6);

        assert!(state.scroll_pages(1));
        assert_eq!(state.offset().1, 6);
        assert!(state.scroll_pages(2));
        assert_eq!(state.offset().1, 18);
        assert!(state.scroll_pages(9));
        assert_eq!(state.offset().1, 34);
        assert!(!state.scroll_pages(1));
        assert!(state.scroll_pages(-1));
        assert_eq!(state.offset().1, 28);
        assert!(state.scroll_pages(-99));
        assert_eq!(state.offset().1, 0);
        assert!(!state.scroll_pages(-1));

        let mut fitting = scrolling(20, 3, 20, 6);
        assert_eq!(fitting.page_lines(), 6);
        assert!(!fitting.scroll_pages(1));
        assert_eq!(fitting.offset(), (0, 0));
    }

    #[test]
    fn wheel_units_convert_to_cells_before_clamping() {
        let mut state = scrolling(20, 20, 10, 5);
        assert_eq!(
            state.apply(&wheel(MouseScrollUnit::Line, 0.0, 2.0)),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset(), (0, 2));

        // 16 px across is two 8 px cells; 32 px down is two 16 px lines.
        assert_eq!(
            state.apply(&wheel(MouseScrollUnit::Pixel, 16.0, 32.0)),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset(), (2, 4));

        assert_eq!(
            state.apply(&wheel(MouseScrollUnit::Line, 0.0, 99.0)),
            WheelDisposition::ContentConsumed
        );
        assert_eq!(state.offset(), (2, 15));
        assert_eq!(
            state.apply(&wheel(MouseScrollUnit::Line, 0.0, 1.0)),
            WheelDisposition::BubbleToWorkspace
        );
    }

    #[test]
    fn offsets_project_through_the_state_metrics_into_pixels() {
        let mut state = scrolling(20, 20, 10, 5);
        assert!(state.scroll_by(2, 3));
        assert_eq!(state.offset_px(), Vec2::new(16.0, 48.0));

        let mut custom = ScrollState::with_metrics(20.0, 10.0);
        custom.set_extent(20, 20, 10, 5);
        assert!(custom.scroll_by(1, 1));
        assert_eq!(custom.offset_px(), Vec2::new(10.0, 20.0));
        assert_eq!(custom.line_height(), 20.0);
        assert_eq!(custom.cell_width(), 10.0);
    }

    /// Spawn one viewport under a fresh root and return the app.
    fn app_with_viewport(state: ScrollState, lines: Vec<String>) -> App {
        let mut app = App::new();
        let theme = BevyTheme::default();
        app.add_systems(Startup, move |mut commands: Commands| {
            commands.spawn(Node::default()).with_children(|parent| {
                spawn(parent, &theme, &state, lines.clone());
            });
        });
        app.update();
        app
    }

    #[test]
    fn spawn_builds_a_clipped_viewport_with_one_text_node_per_line() {
        let mut state = scrolling(20, 20, 10, 5);
        assert!(state.scroll_lines(2));
        let mut app = app_with_viewport(
            state,
            vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()],
        );
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<ScrollViewport>>();
        let viewport = query.single(app.world()).expect("one scroll viewport");

        let world = app.world();
        let node = world.get::<Node>(viewport).expect("viewport node");
        assert_eq!(node.overflow, Overflow::scroll());
        assert_eq!(node.width, Val::Percent(100.0));
        assert_eq!(node.height, Val::Percent(100.0));
        assert_eq!(world.get::<ScrollState>(viewport), Some(&state));
        assert!(world.get::<RelativeCursorPosition>(viewport).is_some());

        let position = world.get::<ScrollPosition>(viewport).expect("scroll position");
        assert_eq!(position.offset_x, 0.0);
        assert_eq!(position.offset_y, 2.0 * LINE_HEIGHT);

        let children = world.get::<Children>(viewport).expect("content child");
        let content = children
            .iter()
            .find(|child| world.get::<ScrollContent>(*child).is_some())
            .expect("scroll content");
        let lines = world.get::<Children>(content).expect("text lines");
        let texts: Vec<&str> = lines
            .iter()
            .map(|line| world.get::<Text>(line).expect("line text").as_str())
            .collect();
        assert_eq!(texts, ["alpha", "beta", "gamma"]);

        let font = world.get::<TextFont>(lines[0]).expect("line font");
        assert_eq!(font.font_size, BevyTheme::default().metrics.body_size);
        assert!(matches!(font.line_height, LineHeight::Px(px) if px == LINE_HEIGHT));
    }

    #[test]
    fn syncing_projects_a_changed_state_into_the_bevy_offset() {
        let mut app = App::new();
        app.add_systems(Update, sync_scroll_position_system);
        let viewport = app
            .world_mut()
            .spawn((scrolling(20, 20, 10, 5), ScrollPosition::default()))
            .id();
        app.update();

        assert!(app
            .world_mut()
            .get_mut::<ScrollState>(viewport)
            .expect("scroll state")
            .scroll_lines(3));
        app.update();

        let position = app.world().get::<ScrollPosition>(viewport).expect("position");
        assert_eq!(position.offset_x, 0.0);
        assert_eq!(position.offset_y, 3.0 * LINE_HEIGHT);
    }

    #[test]
    fn the_wheel_scrolls_only_the_viewport_under_the_cursor() {
        let mut app = App::new();
        app.add_plugins(ScrollPlugin);
        let state = scrolling(20, 20, 10, 5);
        let under_cursor = app
            .world_mut()
            .spawn((state, hovered(), ScrollPosition::default()))
            .id();
        let elsewhere = app
            .world_mut()
            .spawn((state, ScrollPosition::default()))
            .id();

        app.world_mut()
            .send_event(wheel(MouseScrollUnit::Line, 0.0, 3.0));
        app.update();

        let world = app.world();
        let moved = world.get::<ScrollState>(under_cursor).expect("state");
        assert_eq!(moved.offset(), (0, 3));
        assert_eq!(world.get::<ScrollState>(elsewhere).expect("state").offset(), (0, 0));
        let position = world
            .get::<ScrollPosition>(under_cursor)
            .expect("position");
        assert_eq!(position.offset_y, 3.0 * LINE_HEIGHT);
    }

    #[test]
    fn the_wheel_stops_at_the_content_edge_and_fitting_content_never_moves() {
        let mut app = App::new();
        app.add_event::<MouseWheel>();
        app.add_systems(Update, wheel_scroll_system);
        let long = app
            .world_mut()
            .spawn((scrolling(20, 20, 10, 5), hovered()))
            .id();

        app.world_mut()
            .send_event(wheel(MouseScrollUnit::Line, 0.0, 99.0));
        app.update();
        assert_eq!(
            app.world()
                .get::<ScrollState>(long)
                .expect("state")
                .offset(),
            (0, 15)
        );

        app.world_mut()
            .send_event(wheel(MouseScrollUnit::Line, 0.0, 1.0));
        app.update();
        assert_eq!(
            app.world()
                .get::<ScrollState>(long)
                .expect("state")
                .offset(),
            (0, 15)
        );

        let fitting = app
            .world_mut()
            .spawn((scrolling(20, 3, 10, 5), hovered()))
            .id();
        app.world_mut()
            .send_event(wheel(MouseScrollUnit::Line, 0.0, 2.0));
        app.update();
        assert_eq!(
            app.world()
                .get::<ScrollState>(fitting)
                .expect("state")
                .offset(),
            (0, 0)
        );
    }
}
