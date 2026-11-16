//! Form painter: labeled input controls drawn in terminal cells.
//!
//! Core owns the fields, values, focus, text editing, stepping, and the
//! select popup ([`panel_kit_core::widgets::forms`]). This module paints a
//! [`FormModel`] with its [`FormState`] and returns the [`FormLayout`] that
//! resolves a pressed cell to a core [`FormHit`]; the host feeds that, and
//! the keys [`crate::input`] adapts, back into core's `reduce`.
//!
//! A control is a bracketed run (`[value   ]`), the cell analogue of the web
//! control's 1px edge; the focused field carries a `›` marker and bold edge.

use panel_kit_core::widgets::forms::{
    self, ControlModel, FieldModel, FormAction, FormHit, FormInput, FormModel, FormState, KeyRoute,
};
use panel_kit_core::KeyChord;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

use crate::{ResolvedTuiTheme, TerminalCharset};

/// Cells a slider's value readout occupies, right-aligned.
const SLIDER_VALUE_W: u16 = 4;
/// Cells a number box's value occupies between its brackets.
const NUMBER_VALUE_W: u16 = 5;
/// Narrowest control run worth painting; a note yields to it.
const CONTROL_MIN_W: u16 = 6;

/// What a painted run of cells is, for hit resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormPart {
    /// The body of a field's control.
    Control(usize),
    /// One radio option, or one option of the open select popup.
    Choice {
        /// Field index.
        field: usize,
        /// Option index.
        option: usize,
    },
    /// A slider's track; the pressed column gives the fraction.
    Track(usize),
    /// A number box's decrement (`-1`) or increment (`1`) cell.
    Stepper {
        /// Field index.
        field: usize,
        /// Steps to move.
        delta: i32,
    },
}

/// One painted run of cells and what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormZone {
    /// The cells the part occupies.
    pub rect: Rect,
    /// What they are.
    pub part: FormPart,
}

/// Where a painted form's interactive parts landed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FormLayout {
    zones: Vec<FormZone>,
}

impl FormLayout {
    /// The painted parts, in paint order; the open popup's options are last.
    pub fn zones(&self) -> &[FormZone] {
        &self.zones
    }

    /// The first zone that is `part`.
    pub fn zone(&self, part: FormPart) -> Option<Rect> {
        self.zones
            .iter()
            .find(|zone| zone.part == part)
            .map(|zone| zone.rect)
    }

    /// Resolve a pressed cell to the core hit it means. The part painted
    /// last wins, so the open popup covers the rows beneath it.
    pub fn hit(&self, column: u16, row: u16) -> FormHit {
        let zone = self.zones.iter().rev().find(|zone| {
            let r = zone.rect;
            column >= r.x && column < r.right() && row >= r.y && row < r.bottom()
        });
        match zone.map(|zone| (zone.part, zone.rect)) {
            Some((FormPart::Control(field), _)) => FormHit::Control(field),
            Some((FormPart::Choice { field, option }, _)) => FormHit::Choice { field, option },
            Some((FormPart::Stepper { field, delta }, _)) => FormHit::Stepper { field, delta },
            Some((FormPart::Track(field), rect)) => {
                let span = f64::from(rect.width.saturating_sub(1).max(1));
                FormHit::Track {
                    field,
                    fraction: (f64::from(column - rect.x) / span).clamp(0.0, 1.0),
                }
            }
            None => FormHit::Outside,
        }
    }
}

/// The glyphs one charset draws controls with.
struct Glyphs {
    mark: char,
    dot: char,
    thumb: char,
    fill: char,
    track: char,
    chevron: char,
    focus: char,
    edge: char,
    lit_edge: char,
}

impl Glyphs {
    fn of(charset: TerminalCharset) -> Self {
        match charset {
            TerminalCharset::Unicode => Self {
                mark: '■',
                dot: '•',
                thumb: '■',
                fill: '━',
                track: '─',
                chevron: '▾',
                focus: '›',
                edge: '│',
                lit_edge: '▌',
            },
            TerminalCharset::Ascii => Self {
                mark: 'x',
                dot: '*',
                thumb: '#',
                fill: '=',
                track: '-',
                chevron: 'v',
                focus: '>',
                edge: '|',
                lit_edge: '>',
            },
        }
    }
}

/// Rows a field occupies.
fn field_rows(field: &FieldModel) -> u16 {
    match &field.control {
        ControlModel::TextArea { rows, .. } => (*rows).max(1),
        ControlModel::Radio { options, .. } => (options.len() as u16).max(1),
        _ => 1,
    }
}

/// Paint `model` into `area` and return where its parts landed.
///
/// One field per row: key column, control, muted note. A form taller than
/// `area` scrolls so the focused field (and its open popup) stays in view.
pub fn form(
    frame: &mut Frame,
    area: Rect,
    theme: &ResolvedTuiTheme,
    charset: TerminalCharset,
    model: &FormModel,
    state: &FormState,
) -> FormLayout {
    let area = area.intersection(frame.area());
    let mut painter = Painter {
        buf: frame.buffer_mut(),
        area,
        theme,
        glyphs: Glyphs::of(charset),
        state,
        layout: FormLayout::default(),
    };
    painter.paint(model);
    painter.layout
}

/// One form as a terminal host holds it: the model, its interaction state,
/// and where it was last painted. Presses and key chords go through core's
/// `reduce`; the returned actions are the host's to act on.
#[derive(Clone, Debug, PartialEq)]
pub struct FormHost {
    /// Fields and values.
    pub model: FormModel,
    /// Focus, caret, and popup state.
    pub state: FormState,
    area: Rect,
    layout: FormLayout,
}

impl FormHost {
    /// A host for `model` with nothing focused and nothing painted.
    pub fn new(model: FormModel) -> Self {
        Self {
            model,
            state: FormState::default(),
            area: Rect::default(),
            layout: FormLayout::default(),
        }
    }

    /// Paint the form into `area` and keep its layout for [`Self::press`].
    pub fn draw(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        theme: &ResolvedTuiTheme,
        charset: TerminalCharset,
    ) {
        self.area = area.intersection(frame.area());
        self.layout = form(frame, area, theme, charset, &self.model, &self.state);
    }

    /// Forget the painted area: a frame that does not draw the form leaves
    /// nothing to press.
    pub fn clear(&mut self) {
        self.area = Rect::default();
        self.layout = FormLayout::default();
    }

    /// Whether the cell lies in the area the form was last painted into.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        let r = self.area;
        column >= r.x && column < r.right() && row >= r.y && row < r.bottom()
    }

    /// Where the form's parts were last painted.
    pub fn layout(&self) -> &FormLayout {
        &self.layout
    }

    /// A press on the cell `(column, row)` of the painted form.
    pub fn press(&mut self, column: u16, row: u16) -> Vec<FormAction> {
        let hit = self.layout.hit(column, row);
        forms::reduce(&mut self.model, &mut self.state, FormInput::Press(hit))
    }

    /// A press that landed anywhere else: closes the popup and drops focus.
    pub fn press_outside(&mut self) -> Vec<FormAction> {
        forms::reduce(&mut self.model, &mut self.state, FormInput::Press(FormHit::Outside))
    }

    /// Move focus to the first enabled field, for a host's "enter the form"
    /// binding. Does nothing while a field already holds focus.
    pub fn enter(&mut self) -> Vec<FormAction> {
        if self.state.focus.is_some() {
            return Vec::new();
        }
        forms::reduce(&mut self.model, &mut self.state, FormInput::FocusNext)
    }

    /// Route a key chord. `None` leaves the chord to the host; `Some` means
    /// the form took it (the actions may be empty).
    pub fn key(&mut self, chord: KeyChord) -> Option<Vec<FormAction>> {
        match forms::route_chord(&self.model, &self.state, chord) {
            KeyRoute::Input(input) => Some(forms::reduce(&mut self.model, &mut self.state, input)),
            KeyRoute::Swallowed => Some(Vec::new()),
            KeyRoute::Host => None,
        }
    }
}

struct Painter<'a> {
    buf: &'a mut Buffer,
    area: Rect,
    theme: &'a ResolvedTuiTheme,
    glyphs: Glyphs,
    state: &'a FormState,
    layout: FormLayout,
}

/// Colours and weight of one field's control, by state.
#[derive(Clone, Copy)]
struct Tone {
    edge: Style,
    text: Style,
    muted: Style,
    live: Style,
}

impl Painter<'_> {
    fn paint(&mut self, model: &FormModel) {
        if self.area.width == 0 || self.area.height == 0 {
            return;
        }
        let key_w = model
            .fields
            .iter()
            .map(|field| field.label.chars().count() as u16)
            .max()
            .unwrap_or(0)
            .min(self.area.width / 3);
        // One cell after the key column holds the focus marker.
        let control_x = self.area.x + key_w + 1;
        let control_w = self.area.right().saturating_sub(control_x);

        let mut tops = Vec::with_capacity(model.fields.len());
        let mut total: u16 = 0;
        for field in &model.fields {
            tops.push(total);
            total = total.saturating_add(field_rows(field));
        }
        let offset = self.scroll_offset(model, &tops, total);

        let mut popup = None;
        for (index, field) in model.fields.iter().enumerate() {
            let rows = field_rows(field);
            let top = i32::from(tops[index]) - i32::from(offset);
            if top + i32::from(rows) <= 0 || top >= i32::from(self.area.height) {
                continue;
            }
            let y = i32::from(self.area.y) + top;
            self.field(index, field, key_w, control_x, control_w, y, rows);
            if self.state.popup() == Some(index) {
                popup = Some((index, field, y));
            }
        }
        if let Some((index, field, y)) = popup {
            let width = self.control_run(field, control_w);
            self.popup(index, field, control_x, width, y);
        }
    }

    /// Rows scrolled off the top so the focused field and popup are in view.
    fn scroll_offset(&self, model: &FormModel, tops: &[u16], total: u16) -> u16 {
        let Some(focus) = self.state.focus.filter(|index| *index < model.fields.len()) else {
            return 0;
        };
        if total <= self.area.height {
            return 0;
        }
        let field = &model.fields[focus];
        let popup_rows = if self.state.open {
            field.control.options().len() as u16
        } else {
            0
        };
        let bottom = tops[focus] + field_rows(field) + popup_rows;
        bottom
            .saturating_sub(self.area.height)
            .min(tops[focus])
            .min(total - self.area.height)
    }

    fn tone(&self, index: usize, field: &FieldModel) -> Tone {
        let theme = self.theme;
        let fg = |color: Color| Style::default().fg(color);
        if field.disabled {
            return Tone {
                edge: fg(theme.divider),
                text: fg(theme.text_muted),
                muted: fg(theme.text_muted),
                live: fg(theme.text_muted),
            };
        }
        let edge = if self.state.is_focused(index) {
            fg(theme.focus_ring).add_modifier(Modifier::BOLD)
        } else {
            fg(theme.border)
        };
        Tone {
            edge,
            text: fg(theme.text),
            muted: fg(theme.text_muted),
            live: fg(theme.highlight),
        }
    }

    /// Cells the bracketed control run takes, leaving room for the note.
    fn control_run(&self, field: &FieldModel, control_w: u16) -> u16 {
        let note = field.note.chars().count() as u16;
        if note > 0 && control_w > note + 1 + CONTROL_MIN_W {
            control_w - note - 1
        } else {
            control_w
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn field(
        &mut self,
        index: usize,
        field: &FieldModel,
        key_w: u16,
        control_x: u16,
        control_w: u16,
        y: i32,
        rows: u16,
    ) {
        let tone = self.tone(index, field);
        self.text(self.area.x, y, key_w, &field.label.to_uppercase(), tone.muted);
        if self.state.is_focused(index) {
            let marker = self.glyphs.focus.to_string();
            self.text(control_x - 1, y, 1, &marker, tone.edge);
        }
        let run = self.control_run(field, control_w);
        let end = match &field.control {
            ControlModel::Text { .. } => self.text_box(index, field, control_x, run, y, tone),
            ControlModel::TextArea { .. } => {
                self.text_area(index, field, control_x, run, y, rows, tone)
            }
            ControlModel::Number { .. } => self.number(index, field, control_x, y, tone),
            ControlModel::Slider { .. } => self.slider(index, field, control_x, run, y, tone),
            ControlModel::Checkbox { checked } => {
                let mark = if *checked { self.glyphs.mark } else { ' ' };
                self.cell(control_x, y, '[', tone.edge);
                self.cell(control_x + 1, y, mark, tone.live);
                self.cell(control_x + 2, y, ']', tone.edge);
                self.zone(control_x, y, 3, 1, FormPart::Control(index));
                control_x + 3
            }
            ControlModel::Switch { on } => self.switch(index, *on, control_x, y, tone),
            ControlModel::Radio { .. } => self.radio(index, field, control_x, run, y, tone),
            ControlModel::Select { .. } => self.select(index, field, control_x, run, y, tone),
            ControlModel::Button { text } => {
                let caption = format!("[ {} ]", text.to_uppercase());
                let width = (caption.chars().count() as u16).min(run);
                self.text(control_x, y, width, &caption, tone.edge);
                let inner = width.saturating_sub(4);
                self.text(control_x + 2, y, inner, &text.to_uppercase(), tone.muted);
                self.zone(control_x, y, width, 1, FormPart::Control(index));
                control_x + width
            }
        };
        if !field.note.is_empty() {
            let room = self.area.right().saturating_sub(end + 1);
            self.text(end + 1, y, room, &field.note, tone.muted);
        }
    }

    /// `[value     ]` with the caret cell lit while focused.
    fn text_box(
        &mut self,
        index: usize,
        field: &FieldModel,
        x: u16,
        width: u16,
        y: i32,
        tone: Tone,
    ) -> u16 {
        let inner = width.saturating_sub(2);
        self.brackets(x, y, width, tone);
        let value: Vec<char> = field.control.display().chars().collect();
        let focused = self.state.is_focused(index);
        if value.is_empty() && !focused {
            self.text(x + 1, y, inner, field.control.placeholder(), tone.muted);
        } else {
            let caret = self.state.caret.min(value.len());
            // Keep the caret cell inside the box.
            let start = if focused {
                (caret + 1).saturating_sub(inner as usize)
            } else {
                0
            };
            let shown: String = value.iter().skip(start).take(inner as usize).collect();
            self.text(x + 1, y, inner, &shown, tone.text);
            if focused {
                let at = value.get(caret).copied().unwrap_or(' ');
                self.caret(x + 1 + (caret - start) as u16, y, at);
            }
        }
        self.zone(x, y, width, 1, FormPart::Control(index));
        x + width
    }

    #[allow(clippy::too_many_arguments)]
    fn text_area(
        &mut self,
        index: usize,
        field: &FieldModel,
        x: u16,
        width: u16,
        y: i32,
        rows: u16,
        tone: Tone,
    ) -> u16 {
        let inner = width.saturating_sub(2);
        let value = field.control.display();
        let focused = self.state.is_focused(index);
        let lines: Vec<&str> = value.split('\n').collect();
        // Caret as (line, column) within the value.
        let (caret_line, caret_col) = {
            let before: String = value.chars().take(self.state.caret).collect();
            let line = before.matches('\n').count();
            let col = before.rsplit('\n').next().map_or(0, |tail| tail.chars().count());
            (line, col)
        };
        let first = if focused {
            (caret_line + 1).saturating_sub(rows as usize)
        } else {
            0
        };
        for row in 0..rows {
            let line_y = y + i32::from(row);
            self.brackets(x, line_y, width, tone);
            let line = lines.get(first + row as usize).copied();
            if value.is_empty() && !focused && row == 0 {
                self.text(x + 1, line_y, inner, field.control.placeholder(), tone.muted);
            } else if let Some(line) = line {
                self.text(x + 1, line_y, inner, line, tone.text);
            }
            if focused && first + row as usize == caret_line && (caret_col as u16) < inner {
                let at = line.and_then(|line| line.chars().nth(caret_col)).unwrap_or(' ');
                self.caret(x + 1 + caret_col as u16, line_y, at);
            }
        }
        self.zone(x, y, width, rows, FormPart::Control(index));
        x + width
    }

    /// `[    3] - +`: the value box and its two stepper cells.
    fn number(&mut self, index: usize, field: &FieldModel, x: u16, y: i32, tone: Tone) -> u16 {
        let width = NUMBER_VALUE_W + 2;
        self.brackets(x, y, width, tone);
        let value = match &self.state.draft {
            Some(draft) if self.state.is_focused(index) => draft.clone(),
            _ => field.control.display(),
        };
        let shown: String = value.chars().rev().take(NUMBER_VALUE_W as usize).collect();
        let shown: String = shown.chars().rev().collect();
        let pad = NUMBER_VALUE_W - shown.chars().count() as u16;
        self.text(x + 1 + pad, y, NUMBER_VALUE_W - pad, &shown, tone.text);
        self.zone(x, y, width, 1, FormPart::Control(index));
        let minus = x + width + 1;
        self.cell(minus, y, '-', tone.muted);
        self.cell(minus + 2, y, '+', tone.muted);
        self.zone(minus - 1, y, 2, 1, FormPart::Stepper { field: index, delta: -1 });
        self.zone(minus + 1, y, 2, 1, FormPart::Stepper { field: index, delta: 1 });
        minus + 3
    }

    /// `━━━■───  60`: the filled track, thumb, and right-aligned value.
    fn slider(
        &mut self,
        index: usize,
        field: &FieldModel,
        x: u16,
        width: u16,
        y: i32,
        tone: Tone,
    ) -> u16 {
        let track = width.saturating_sub(SLIDER_VALUE_W + 1).max(1);
        let fraction = field.control.fraction().unwrap_or(0.0);
        let thumb = (fraction * f64::from(track - 1)).round() as u16;
        let rest = if field.disabled {
            tone.edge
        } else {
            Style::default().fg(self.theme.border)
        };
        for cell in 0..track {
            let (glyph, style) = match cell.cmp(&thumb) {
                std::cmp::Ordering::Less => (self.glyphs.fill, tone.text),
                std::cmp::Ordering::Equal if self.state.is_focused(index) => {
                    (self.glyphs.thumb, tone.edge)
                }
                std::cmp::Ordering::Equal => (self.glyphs.thumb, tone.text),
                std::cmp::Ordering::Greater => (self.glyphs.track, rest),
            };
            self.cell(x + cell, y, glyph, style);
        }
        self.zone(x, y, track, 1, FormPart::Track(index));
        let value = field.control.display();
        let shown: String = value.chars().take(SLIDER_VALUE_W as usize).collect();
        let pad = SLIDER_VALUE_W - shown.chars().count() as u16;
        self.text(x + track + 1 + pad, y, SLIDER_VALUE_W - pad, &shown, tone.text);
        x + track + 1 + SLIDER_VALUE_W
    }

    /// `[■ ] OFF` / `[ ■] ON`.
    fn switch(&mut self, index: usize, on: bool, x: u16, y: i32, tone: Tone) -> u16 {
        self.brackets(x, y, 4, tone);
        let (at, style, caption, caption_style) = if on {
            (x + 2, tone.live, "ON", tone.text)
        } else {
            (x + 1, tone.muted, "OFF", tone.muted)
        };
        self.cell(at, y, self.glyphs.thumb, style);
        self.zone(x, y, 4, 1, FormPart::Control(index));
        self.text(x + 5, y, 3, caption, caption_style);
        x + 5 + caption.len() as u16
    }

    /// One `(•) label` row per option.
    fn radio(
        &mut self,
        index: usize,
        field: &FieldModel,
        x: u16,
        width: u16,
        y: i32,
        tone: Tone,
    ) -> u16 {
        let chosen = field.control.selected_index();
        let mut end = x;
        for (option, choice) in field.control.options().iter().enumerate() {
            let row = y + option as i32;
            let dot = if chosen == Some(option) { self.glyphs.dot } else { ' ' };
            self.cell(x, row, '(', tone.edge);
            self.cell(x + 1, row, dot, tone.live);
            self.cell(x + 2, row, ')', tone.edge);
            let label_w = width.saturating_sub(4);
            self.text(x + 4, row, label_w, &choice.label, tone.text);
            let used = 4 + (choice.label.chars().count() as u16).min(label_w);
            self.zone(x, row, used, 1, FormPart::Choice { field: index, option });
            if option == 0 {
                end = x + used;
            }
        }
        end
    }

    /// `[label      ▾]`; the edge and chevron light while the popup is open.
    fn select(
        &mut self,
        index: usize,
        field: &FieldModel,
        x: u16,
        width: u16,
        y: i32,
        tone: Tone,
    ) -> u16 {
        let open = self.state.popup() == Some(index);
        let edge = if open { tone.live.add_modifier(Modifier::BOLD) } else { tone.edge };
        self.brackets(x, y, width, Tone { edge, ..tone });
        let inner = width.saturating_sub(4);
        let label = field.control.display();
        if label.is_empty() {
            self.text(x + 1, y, inner, field.control.placeholder(), tone.muted);
        } else {
            self.text(x + 1, y, inner, &label, tone.text);
        }
        let chevron = if open { tone.live } else { tone.muted };
        self.cell(x + width.saturating_sub(2), y, self.glyphs.chevron, chevron);
        self.zone(x, y, width, 1, FormPart::Control(index));
        x + width
    }

    /// The open select's options, under the trigger, or above it when the
    /// rows below cannot hold them.
    fn popup(&mut self, index: usize, field: &FieldModel, x: u16, width: u16, trigger_y: i32) {
        let options = field.control.options();
        let rows = options.len() as i32;
        let below = trigger_y + 1;
        let top = if below + rows <= i32::from(self.area.bottom()) {
            below
        } else {
            (trigger_y - rows).max(i32::from(self.area.y))
        };
        let chosen = field.control.selected_index();
        let fill = Style::default().bg(self.theme.background);
        let inner = width.saturating_sub(2);
        for (option, choice) in options.iter().enumerate() {
            let y = top + option as i32;
            for cell in 0..width {
                self.cell(x + cell, y, ' ', fill);
            }
            let highlighted = self.state.highlighted == Some(option);
            let (left, left_style) = if highlighted {
                (self.glyphs.lit_edge, fill.fg(self.theme.highlight))
            } else {
                (self.glyphs.edge, fill.fg(self.theme.border))
            };
            self.cell(x, y, left, left_style);
            self.cell(x + width.saturating_sub(1), y, self.glyphs.edge, fill.fg(self.theme.border));
            let text = if chosen == Some(option) {
                fill.fg(self.theme.highlight)
            } else if highlighted {
                fill.fg(self.theme.text).add_modifier(Modifier::BOLD)
            } else {
                fill.fg(self.theme.text)
            };
            self.text(x + 1, y, inner, &choice.label, text);
            self.zone(x, y, width, 1, FormPart::Choice { field: index, option });
        }
    }

    fn brackets(&mut self, x: u16, y: i32, width: u16, tone: Tone) {
        if width < 2 {
            return;
        }
        let fill = Style::default().bg(self.theme.background);
        for cell in 1..width - 1 {
            self.cell(x + cell, y, ' ', fill);
        }
        self.cell(x, y, '[', tone.edge);
        self.cell(x + width - 1, y, ']', tone.edge);
    }

    /// The caret cell: the character under it on the highlight.
    fn caret(&mut self, x: u16, y: i32, under: char) {
        let style = Style::default()
            .fg(self.theme.inverse_text)
            .bg(self.theme.highlight);
        self.cell(x, y, under, style);
    }

    fn zone(&mut self, x: u16, y: i32, width: u16, height: u16, part: FormPart) {
        let top = y.max(i32::from(self.area.y));
        let bottom = (y + i32::from(height)).min(i32::from(self.area.bottom()));
        let right = (x + width).min(self.area.right());
        if bottom <= top || right <= x {
            return;
        }
        self.layout.zones.push(FormZone {
            rect: Rect::new(x, top as u16, right - x, (bottom - top) as u16),
            part,
        });
    }

    fn text(&mut self, x: u16, y: i32, max_width: u16, text: &str, style: Style) {
        for (offset, ch) in text.chars().take(max_width as usize).enumerate() {
            self.cell(x + offset as u16, y, ch, style);
        }
    }

    /// Write one cell, clipped to the form's area. The style patches the
    /// cell, so a foreground-only style keeps the fill beneath it.
    fn cell(&mut self, x: u16, y: i32, ch: char, style: Style) {
        let inside = x >= self.area.x
            && x < self.area.right()
            && y >= i32::from(self.area.y)
            && y < i32::from(self.area.bottom());
        if inside {
            let cell = &mut self.buf[(x, y as u16)];
            cell.set_char(ch);
            cell.set_style(style);
        }
    }
}
