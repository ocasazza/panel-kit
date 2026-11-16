//! The form painter draws the shared canary form in cells, resolves presses
//! to core hits, and drives core's reducer through [`FormHost`].

#[allow(dead_code)]
#[path = "../../../examples/support/canary_content.rs"]
mod canary_content;

use canary_content::canary_inputs;
use panel_kit_core::widgets::forms::{
    ControlModel, FieldValue, FormAction, FormHit, FormModel, FormState,
};
use panel_kit_core::{Key, KeyChord};
use panel_kit_tui::forms::{form, FormHost, FormLayout};
use panel_kit_tui::{ResolvedTuiTheme, TerminalCharset};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Modifier;
use ratatui::Terminal;

/// The Inputs panel body in the native canary: 29 columns.
const AREA: Rect = Rect::new(0, 0, 29, 20);

// Field indices of `canary_inputs`.
const HOST: usize = 0;
const REPLICAS: usize = 5;
const THRESHOLD: usize = 6;
const DRAIN: usize = 7;
const TIER: usize = 9;
const REGION: usize = 10;

fn paint(
    area: Rect,
    charset: TerminalCharset,
    model: &FormModel,
    state: &FormState,
) -> (Buffer, FormLayout) {
    let theme = ResolvedTuiTheme::default();
    let mut terminal = Terminal::new(TestBackend::new(area.right(), area.bottom())).unwrap();
    let mut layout = FormLayout::default();
    terminal
        .draw(|frame| layout = form(frame, area, &theme, charset, model, state))
        .unwrap();
    (terminal.backend().buffer().clone(), layout)
}

fn row(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn chord(key: Key) -> KeyChord {
    KeyChord { key, shift: false, alt: false, ctrl: false, meta: false }
}

#[test]
fn the_canary_form_paints_one_face_per_control() {
    let (buffer, _) = paint(AREA, TerminalCharset::Unicode, &canary_inputs(), &FormState::default());
    let expected = [
        "HOST      [pdx-01           ]",
        "TOKEN     [•••••••          ]",
        "FILTER    [label or tag     ]",
        "SINCE     [2026-11-01       ]",
        "NOTES     [drain before     ]",
        "          [the upgrade      ]",
        "REPLICAS  [    3] - + pods",
        "THRESHOLD ━━━━━━━■────   60 %",
        "DRAIN     [■]",
        "ALERTS    [■ ] OFF",
        "TIER      ( ) hot",
        "          (•) warm",
        "          ( ) cold",
        "REGION    [choose region   ▾]",
        "OWNER     [platform  ] locked",
        "          [ APPLY ]",
    ];
    for (y, line) in expected.iter().enumerate() {
        assert_eq!(row(&buffer, y as u16), *line, "row {y}");
    }
}

#[test]
fn the_ascii_charset_replaces_every_non_ascii_glyph() {
    let mut model = canary_inputs();
    model.fields.remove(1); // the password's mask glyph is core's, not the charset's
    let state = FormState { focus: Some(REGION - 1), open: true, highlighted: Some(0), ..FormState::default() };
    let (buffer, _) = paint(AREA, TerminalCharset::Ascii, &model, &state);
    let painted: String = (0..AREA.height).map(|y| row(&buffer, y)).collect();
    assert!(painted.is_ascii(), "non-ASCII glyph in {painted:?}");
    assert_eq!(row(&buffer, 6), "THRESHOLD =======#----   60 %");
    assert_eq!(row(&buffer, 7), "DRAIN     [x]");
    assert_eq!(row(&buffer, 10), "          (*) warm");
    assert_eq!(row(&buffer, 12), "REGION   >[choose region   v]");
}

#[test]
fn placeholders_notes_and_disabled_fields_are_muted_and_live_marks_are_lit() {
    let theme = ResolvedTuiTheme::default();
    let (buffer, _) = paint(AREA, TerminalCharset::Unicode, &canary_inputs(), &FormState::default());
    assert_eq!(buffer[(11, 0)].fg, theme.text, "a value");
    assert_eq!(buffer[(11, 2)].fg, theme.text_muted, "a placeholder");
    assert_eq!(buffer[(22, 6)].fg, theme.text_muted, "a note");
    assert_eq!(buffer[(11, 14)].fg, theme.text_muted, "a disabled value");
    assert_eq!(buffer[(10, 14)].fg, theme.divider, "a disabled edge");
    assert_eq!(buffer[(10, 0)].fg, theme.border, "a resting edge");
    assert_eq!(buffer[(11, 8)].fg, theme.highlight, "the checked mark");
    assert_eq!(buffer[(11, 11)].fg, theme.highlight, "the chosen radio dot");
    assert_eq!(buffer[(11, 9)].fg, theme.text_muted, "the switch thumb, off");
    assert_eq!(buffer[(17, 7)].fg, theme.text, "the slider thumb is a level, not live state");
}

#[test]
fn the_focused_field_carries_the_marker_a_bold_edge_and_the_caret() {
    let theme = ResolvedTuiTheme::default();
    let state = FormState { focus: Some(HOST), caret: 3, ..FormState::default() };
    let (buffer, _) = paint(AREA, TerminalCharset::Unicode, &canary_inputs(), &state);

    assert_eq!(row(&buffer, 0), "HOST     ›[pdx-01           ]");
    assert_eq!(buffer[(10, 0)].fg, theme.focus_ring);
    assert!(buffer[(10, 0)].modifier.contains(Modifier::BOLD));
    let caret = &buffer[(14, 0)];
    assert_eq!((caret.symbol(), caret.bg), ("-", theme.highlight), "the caret sits on the 4th character");
    assert!(!buffer[(10, 1)].modifier.contains(Modifier::BOLD), "other fields stay at rest");
}

#[test]
fn the_open_popup_covers_the_rows_below_its_trigger() {
    let theme = ResolvedTuiTheme::default();
    let mut model = canary_inputs();
    if let ControlModel::Select { value, .. } = &mut model.fields[REGION].control {
        *value = "pdx".into();
    }
    let state = FormState {
        focus: Some(REGION),
        open: true,
        highlighted: Some(1),
        ..FormState::default()
    };
    let (buffer, layout) = paint(AREA, TerminalCharset::Unicode, &model, &state);

    assert_eq!(row(&buffer, 13), "REGION   ›[Portland        ▾]");
    assert_eq!(buffer[(27, 13)].fg, theme.highlight, "the open chevron is lit");
    assert_eq!(row(&buffer, 14), "OWNER     │Portland         │");
    assert_eq!(row(&buffer, 15), "          ▌Ashburn          │");
    assert_eq!(row(&buffer, 16), "          │Frankfurt        │");
    assert_eq!(row(&buffer, 17), "          │Tokyo            │");
    assert_eq!(buffer[(11, 14)].fg, theme.highlight, "the chosen option");
    assert_eq!(buffer[(10, 15)].fg, theme.highlight, "the highlighted option's edge");
    assert_eq!(buffer[(11, 16)].fg, theme.text);
    assert_eq!(layout.hit(12, 15), FormHit::Choice { field: REGION, option: 1 });
}

#[test]
fn a_popup_with_no_room_below_opens_above_its_trigger() {
    let area = Rect::new(0, 0, 29, 15);
    let state = FormState { focus: Some(REGION), open: true, ..FormState::default() };
    let (buffer, layout) = paint(area, TerminalCharset::Unicode, &canary_inputs(), &state);
    // The form scrolls one row so the trigger's row is in view at y = 12.
    assert_eq!(row(&buffer, 12), "REGION   ›[choose region   ▾]");
    assert_eq!(row(&buffer, 11), "          │Tokyo            │");
    assert_eq!(layout.hit(12, 8), FormHit::Choice { field: REGION, option: 0 });
}

#[test]
fn presses_resolve_to_the_part_under_the_cell() {
    let (_, layout) = paint(AREA, TerminalCharset::Unicode, &canary_inputs(), &FormState::default());

    assert_eq!(layout.hit(11, 8), FormHit::Control(DRAIN));
    assert_eq!(layout.hit(15, 12), FormHit::Choice { field: TIER, option: 2 });
    assert_eq!(layout.hit(18, 6), FormHit::Stepper { field: REPLICAS, delta: -1 });
    assert_eq!(layout.hit(20, 6), FormHit::Stepper { field: REPLICAS, delta: 1 });
    assert_eq!(layout.hit(12, 6), FormHit::Control(REPLICAS));
    assert_eq!(layout.hit(27, 8), FormHit::Outside, "right of the checkbox");
    assert_eq!(layout.hit(3, 0), FormHit::Outside, "a key label");

    let fraction = |x: u16| match layout.hit(x, 7) {
        FormHit::Track { field: THRESHOLD, fraction } => fraction,
        other => panic!("{other:?} at column {x}"),
    };
    assert_eq!(fraction(10), 0.0);
    assert_eq!(fraction(21), 1.0);
    let fractions: Vec<f64> = (10..=21).map(fraction).collect();
    assert!(fractions.windows(2).all(|pair| pair[0] < pair[1]), "{fractions:?}");
}

#[test]
fn a_form_taller_than_its_area_scrolls_the_focused_field_into_view() {
    let area = Rect::new(0, 0, 29, 4);
    let last = canary_inputs().fields.len() - 1;
    let state = FormState { focus: Some(last), ..FormState::default() };
    let (buffer, layout) = paint(area, TerminalCharset::Unicode, &canary_inputs(), &state);
    assert_eq!(row(&buffer, 3), "         ›[ APPLY ]");
    assert_eq!(layout.hit(12, 3), FormHit::Control(last));
}

#[test]
fn a_host_drives_the_form_through_presses_and_keys() {
    let theme = ResolvedTuiTheme::default();
    let mut host = FormHost::new(canary_inputs());
    let mut terminal = Terminal::new(TestBackend::new(AREA.width, AREA.height)).unwrap();
    let mut draw = |host: &mut FormHost| {
        terminal
            .draw(|frame| host.draw(frame, AREA, &theme, TerminalCharset::Unicode))
            .unwrap();
        terminal.backend().buffer().clone()
    };

    draw(&mut host);
    assert_eq!(host.key(chord(Key::Char('q'))), None, "an unfocused form leaves keys to the host");

    assert!(host.press(12, 13).is_empty(), "opening the popup changes no value");
    draw(&mut host);
    let actions = host.press(12, 15);
    assert_eq!(
        actions,
        [FormAction::Changed { id: "region".into(), value: FieldValue::Choice("iad".into()) }]
    );
    let buffer = draw(&mut host);
    assert_eq!(row(&buffer, 13), "REGION   ›[Ashburn         ▾]");
    assert_eq!(row(&buffer, 14), "OWNER     [platform  ] locked", "the popup closed");

    host.press(11, 8);
    assert_eq!(
        host.key(chord(Key::Char(' '))),
        Some(vec![FormAction::Changed { id: "drain".into(), value: FieldValue::Bool(true) }]),
        "the press unchecked it; space checks it again"
    );
    assert_eq!(host.key(chord(Key::Char('q'))), Some(Vec::new()), "a focused form swallows bare keys");

    host.press(11, 0);
    host.key(chord(Key::Char('!')));
    let buffer = draw(&mut host);
    assert_eq!(row(&buffer, 0), "HOST     ›[pdx-01!          ]");

    assert!(host.press_outside().is_empty());
    assert_eq!(host.state.focus, None);
    host.clear();
    assert!(!host.contains(12, 13), "an undrawn form has no cells to press");
}
