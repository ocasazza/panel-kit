//! Theme font resolution for Bevy text.
//!
//! The web canary is painted by Chromium, so this resolves the theme's CSS
//! font stack the way Chromium does: families match installed font family
//! names exactly (case-insensitive), `ui-monospace` is skipped (Chromium has
//! no `ui-*` generic families), and `monospace` is the platform's default
//! fixed-pitch family. The matched family's regular and bold faces back
//! [`TextFont::default`](bevy::text::TextFont) and [`BOLD_FONT`].
//!
//! Bevy text has no per-glyph fallback. Glyphs the theme family lacks render
//! through [`fallback_font`], resolved per glyph from the platform families a
//! browser cascades to (Chromium paints the braille spinner frames from Apple
//! Braille and `⟶` from Apple Symbols on macOS); text helpers set those
//! glyphs as spans in the fallback face.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use bevy::asset::{weak_handle, Assets, Handle};
use bevy::text::Font;
use panel_kit_core::theme::ThemeTokens;

/// The bold face of the theme family, for `font-weight >= 600` text.
pub const BOLD_FONT: Handle<Font> = weak_handle!("6f0d2a7c-61f4-4b31-9b6a-2f6c7c1f40b1");

/// Faces covering painted glyphs the theme family lacks, in cascade order.
pub const FALLBACK_FONTS: [Handle<Font>; 3] = [
    weak_handle!("2a9c51e4-0c7b-4f0e-a3d2-8e5f6b1d9c37"),
    weak_handle!("8c3d1b7a-6e2f-4a90-b5d4-1f7e9c2a3b60"),
    weak_handle!("d51e7f20-3a8c-4b6d-9e1f-0c2b4a6d8e93"),
];

/// What the installed theme family resolved to at runtime.
struct Installed {
    /// Horizontal advance of the regular face, in em (monospace).
    advance_em: f32,
    /// Painted glyphs the regular face lacks: their [`FALLBACK_FONTS`] slot
    /// and their advance in that face, in em.
    fallback: HashMap<char, (usize, f32)>,
}

static INSTALLED: RwLock<Option<Installed>> = RwLock::new(None);

/// Pitch assumed for monospace measures before a theme font is installed.
const UNINSTALLED_ADVANCE_EM: f32 = 0.6;

/// The installed theme face's advance width in em, once a theme font is
/// installed. Monospace text lays out on this pitch, as the browser does.
pub fn advance_em() -> Option<f32> {
    INSTALLED.read().ok()?.as_ref().map(|installed| installed.advance_em)
}

/// One CSS `ch` in em: the installed face's `0` advance.
pub fn ch_em() -> f32 {
    advance_em().unwrap_or(UNINSTALLED_ADVANCE_EM)
}

/// Advance of `glyph` in em in the face that paints it, once a theme font is
/// installed.
pub fn glyph_advance_em(glyph: char) -> Option<f32> {
    let installed = INSTALLED.read().ok()?;
    let installed = installed.as_ref()?;
    Some(
        installed
            .fallback
            .get(&glyph)
            .map_or(installed.advance_em, |(_, advance)| *advance),
    )
}

/// The fallback face for `glyph` when the installed theme face lacks it.
pub fn fallback_font(glyph: char) -> Option<Handle<Font>> {
    let installed = INSTALLED.read().ok()?;
    let (slot, _) = *installed.as_ref()?.fallback.get(&glyph)?;
    Some(FALLBACK_FONTS[slot].clone())
}

/// Non-ASCII glyphs panel-kit painters draw: chrome separators and arrows,
/// badge link arrows, list markers, plus core's spinner frames.
pub fn painted_glyphs() -> impl Iterator<Item = char> {
    "·→×—…⟶•●★"
        .chars()
        .chain(panel_kit_core::widgets::spinner::FRAMES.iter().flat_map(|frame| frame.chars()))
}

/// Chromium's default fixed-pitch families, in platform order.
const MONOSPACE_DEFAULTS: [&str; 3] = ["Menlo", "DejaVu Sans Mono", "Liberation Mono"];

/// System families a browser cascades to for symbols the page font lacks.
const SYMBOL_FALLBACKS: [&str; 5] = [
    "Apple Braille",
    "Apple Symbols",
    "DejaVu Sans",
    "Noto Sans Symbols 2",
    "Segoe UI Symbol",
];

/// One face inside an installed font file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FontFace {
    /// The font file.
    pub path: PathBuf,
    /// Face index inside a collection; `0` for a single-face file.
    pub index: u32,
    /// Family name (typographic family when present).
    pub family: String,
    /// OS/2 weight class.
    pub weight: u16,
    /// Whether the face is italic or oblique.
    pub italic: bool,
}

/// The faces resolved for one theme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeFaces {
    /// Upright face closest to weight 400.
    pub regular: FontFace,
    /// Upright face closest to weight 700, when the family has one at 600+.
    pub bold: Option<FontFace>,
    /// Faces covering painted glyphs `regular` lacks, each with the glyphs it
    /// serves, in cascade order. At most [`FALLBACK_FONTS`] faces.
    pub fallback: Vec<(FontFace, Vec<char>)>,
}

/// Resolve the theme's CSS font `stack` against the fonts under `dirs`.
///
/// Returns `None` when no family in the stack is installed.
pub fn resolve_stack(stack: &str, dirs: &[PathBuf]) -> Option<ThemeFaces> {
    let files: Vec<PathBuf> = dirs.iter().flat_map(|dir| font_files(dir, 3)).collect();
    resolve_with(stack, |family| family_faces(&files, family), face_covers)
}

/// [`resolve_stack`] over a family lookup and a glyph-coverage check.
fn resolve_with(
    stack: &str,
    lookup: impl Fn(&str) -> Vec<FontFace>,
    covers: impl Fn(&FontFace, &[char]) -> bool,
) -> Option<ThemeFaces> {
    let faces = stack
        .split(',')
        .map(|family| family.trim().trim_matches(|c| c == '"' || c == '\''))
        .flat_map(|family| match family.to_ascii_lowercase().as_str() {
            "ui-monospace" => Vec::new(),
            "monospace" => MONOSPACE_DEFAULTS.to_vec(),
            _ => vec![family],
        })
        .map(&lookup)
        .find(|faces| faces.iter().any(|face| !face.italic))?;
    let regular = closest_upright(&faces, 400)?;
    let bold = closest_upright(&faces, 700).filter(|face| face.weight >= 600);
    let cascade: Vec<FontFace> = SYMBOL_FALLBACKS
        .iter()
        .flat_map(|family| lookup(family))
        .filter(|face| !face.italic)
        .collect();
    let mut fallback: Vec<(FontFace, Vec<char>)> = Vec::new();
    for glyph in painted_glyphs().filter(|glyph| !covers(&regular, &[*glyph])) {
        let Some(face) = cascade.iter().find(|face| covers(face, &[glyph])) else {
            continue;
        };
        if let Some((_, glyphs)) = fallback.iter_mut().find(|(served, _)| served == face) {
            glyphs.push(glyph);
        } else if fallback.len() < FALLBACK_FONTS.len() {
            fallback.push((face.clone(), vec![glyph]));
        }
    }
    Some(ThemeFaces {
        regular,
        bold,
        fallback,
    })
}

/// Standard system and per-user font directories on macOS and Linux.
pub fn system_font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![
        PathBuf::from("/System/Library/Fonts"),
        PathBuf::from("/Library/Fonts"),
        PathBuf::from("/usr/share/fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        dirs.push(home.join("Library/Fonts"));
        dirs.push(home.join(".local/share/fonts"));
    }
    dirs
}

/// Install the theme's faces: regular as Bevy's default font, bold as
/// [`BOLD_FONT`] (the regular face when the family has no bold), and each
/// fallback face in its [`FALLBACK_FONTS`] slot. Records the regular face's
/// advance and fallback map for [`advance_em`] and [`fallback_font`]. Returns
/// the faces used, or `None` when no family in the stack is installed (the
/// embedded font stays).
pub fn install_theme_font(fonts: &mut Assets<Font>, tokens: &ThemeTokens) -> Option<ThemeFaces> {
    let faces = resolve_stack(&tokens.typography.family, &system_font_dirs())?;
    let regular_bytes = face_bytes(&faces.regular)?;
    let advance_em = advance_of(&regular_bytes, '0')?;
    let regular = Font::try_from_bytes(regular_bytes).ok()?;
    let bold = faces
        .bold
        .as_ref()
        .and_then(|face| Font::try_from_bytes(face_bytes(face)?).ok())
        .unwrap_or_else(|| regular.clone());
    let mut fallback = HashMap::new();
    for (slot, (face, glyphs)) in faces.fallback.iter().enumerate() {
        let Some(bytes) = face_bytes(face) else {
            continue;
        };
        for glyph in glyphs {
            if let Some(advance) = advance_of(&bytes, *glyph) {
                fallback.insert(*glyph, (slot, advance));
            }
        }
        if let Ok(font) = Font::try_from_bytes(bytes) {
            fonts.insert(&FALLBACK_FONTS[slot], font);
        }
    }
    fonts.insert(&Handle::<Font>::default(), regular);
    fonts.insert(&BOLD_FONT, bold);
    if let Ok(mut installed) = INSTALLED.write() {
        *installed = Some(Installed {
            advance_em,
            fallback,
        });
    }
    Some(faces)
}

fn face_bytes(face: &FontFace) -> Option<Vec<u8>> {
    single_face(&std::fs::read(&face.path).ok()?, face.index)
}

/// Advance of `glyph` in em; for `0`, the CSS `ch` and the monospace pitch.
fn advance_of(bytes: &[u8], glyph: char) -> Option<f32> {
    let face = ttf_parser::Face::parse(bytes, 0).ok()?;
    let advance = face.glyph_hor_advance(face.glyph_index(glyph)?)?;
    Some(f32::from(advance) / f32::from(face.units_per_em()))
}

/// The faces of `family` among `files`. File stems prefilter the candidates;
/// the face's own family name decides.
fn family_faces(files: &[PathBuf], family: &str) -> Vec<FontFace> {
    let key = normalize(family);
    files
        .iter()
        .filter(|file| {
            file.file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| normalize(stem).starts_with(&key))
        })
        .flat_map(|file| file_faces(file))
        .filter(|face| face.family.eq_ignore_ascii_case(family))
        .collect()
}

fn closest_upright(faces: &[FontFace], weight: u16) -> Option<FontFace> {
    faces
        .iter()
        .filter(|face| !face.italic)
        .min_by_key(|face| face.weight.abs_diff(weight))
        .cloned()
}

fn file_faces(path: &Path) -> Vec<FontFace> {
    let Ok(data) = std::fs::read(path) else {
        return Vec::new();
    };
    let count = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    (0..count)
        .filter_map(|index| {
            let face = ttf_parser::Face::parse(&data, index).ok()?;
            Some(FontFace {
                path: path.to_path_buf(),
                index,
                family: family_name(&face)?,
                weight: face.weight().to_number(),
                italic: face.is_italic() || face.is_oblique(),
            })
        })
        .collect()
}

/// Typographic family, else family, from Unicode records or, as in Menlo,
/// Mac Roman records (ASCII family names decode byte for byte).
fn family_name(face: &ttf_parser::Face<'_>) -> Option<String> {
    use ttf_parser::name_id::{FAMILY, TYPOGRAPHIC_FAMILY};
    use ttf_parser::PlatformId;
    let named = |id| {
        face.names()
            .into_iter()
            .filter(|name| name.name_id == id)
            .find_map(|name| {
                if name.is_unicode() {
                    name.to_string()
                } else if name.platform_id == PlatformId::Macintosh && name.name.is_ascii() {
                    Some(String::from_utf8_lossy(name.name).into_owned())
                } else {
                    None
                }
            })
    };
    named(TYPOGRAPHIC_FAMILY).or_else(|| named(FAMILY))
}

fn face_covers(face: &FontFace, glyphs: &[char]) -> bool {
    let Ok(data) = std::fs::read(&face.path) else {
        return false;
    };
    let Ok(parsed) = ttf_parser::Face::parse(&data, face.index) else {
        return false;
    };
    glyphs.iter().all(|glyph| parsed.glyph_index(*glyph).is_some())
}

/// The bytes of one face as a standalone font file.
///
/// Bevy loads a collection's last face, so a collection face is rebuilt as a
/// single-face sfnt: its table directory with every table copied after it.
fn single_face(data: &[u8], index: u32) -> Option<Vec<u8>> {
    if data.get(0..4)? != b"ttcf" {
        return (index == 0).then(|| data.to_vec());
    }
    let read_u32 = |at: usize| Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?));
    let read_u16 = |at: usize| Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?));
    let start = read_u32(12 + 4 * index as usize)? as usize;
    let tables = read_u16(start + 4)? as usize;
    let directory = 12 + 16 * tables;
    let mut out = data.get(start..start + directory)?.to_vec();
    for table in 0..tables {
        let record = start + 12 + 16 * table;
        let offset = read_u32(record + 8)? as usize;
        let length = read_u32(record + 12)? as usize;
        let new_offset = u32::try_from(out.len()).ok()?;
        out[12 + 16 * table + 8..12 + 16 * table + 12].copy_from_slice(&new_offset.to_be_bytes());
        out.extend_from_slice(data.get(offset..offset + length)?);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    Some(out)
}

fn normalize(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn font_files(dir: &Path, depth: u8) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for path in entries.flatten().map(|entry| entry.path()) {
        if path.is_dir() {
            if depth > 0 {
                files.extend(font_files(&path, depth - 1));
            }
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc"))
        {
            files.push(path);
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(family: &str, weight: u16, italic: bool) -> FontFace {
        FontFace {
            path: PathBuf::from(format!("{family}-{weight}-{italic}.ttf")),
            index: 0,
            family: family.to_owned(),
            weight,
            italic,
        }
    }

    fn lookup(installed: Vec<FontFace>) -> impl Fn(&str) -> Vec<FontFace> {
        move |family| {
            installed
                .iter()
                .filter(|face| face.family.eq_ignore_ascii_case(family))
                .cloned()
                .collect()
        }
    }

    /// Chromium semantics: `ui-monospace` is no family, a named family needs
    /// an exact family name, and `monospace` is the platform default.
    #[test]
    fn stack_resolves_like_chromium() {
        let installed = vec![
            face("SF NS Mono", 400, false),
            face("JetBrainsMono Nerd Font", 400, false),
            face("Menlo", 400, false),
            face("Menlo", 700, false),
            face("Menlo", 400, true),
        ];
        let all = |_: &FontFace, _: &[char]| true;
        let stack = "ui-monospace, \"SF Mono\", \"JetBrains Mono\", \"Menlo\", monospace";
        let faces = resolve_with(stack, lookup(installed.clone()), all).expect("menlo resolves");
        assert_eq!(faces.regular, face("Menlo", 400, false));
        assert_eq!(faces.bold, Some(face("Menlo", 700, false)));
        assert!(faces.fallback.is_empty(), "a covering family needs no fallback");

        let generic = resolve_with("Consolas, monospace", lookup(installed), all);
        assert_eq!(generic.map(|faces| faces.regular.family), Some("Menlo".to_owned()));
        assert_eq!(resolve_with("Consolas", lookup(Vec::new()), all), None);
    }

    /// A family without a 600+ upright face has no bold, and each glyph the
    /// regular face lacks comes from the first cascade family covering it.
    #[test]
    fn bold_needs_weight_and_fallback_cascades_per_glyph() {
        let installed = vec![
            face("Plain Mono", 400, false),
            face("Plain Mono", 500, false),
            face("Apple Braille", 400, false),
            face("Apple Symbols", 400, false),
        ];
        let braille = |glyph: &char| ('\u{2800}'..='\u{28ff}').contains(glyph);
        let covers = |face: &FontFace, glyphs: &[char]| match face.family.as_str() {
            "Plain Mono" => glyphs.iter().all(|glyph| glyph.is_ascii() || *glyph == '·'),
            "Apple Braille" => glyphs.iter().all(braille),
            _ => glyphs.iter().all(|glyph| *glyph == '⟶'),
        };
        let faces = resolve_with("Plain Mono", lookup(installed), covers).expect("resolves");
        assert_eq!(faces.bold, None);
        let served: Vec<(&str, bool)> = faces
            .fallback
            .iter()
            .map(|(face, glyphs)| (face.family.as_str(), glyphs.iter().all(braille)))
            .collect();
        assert_eq!(served, [("Apple Symbols", false), ("Apple Braille", true)]);
        assert_eq!(faces.fallback[0].1, ['⟶']);
    }

    /// A collection face is rebuilt as a standalone sfnt carrying its own
    /// tables at rewritten offsets.
    #[test]
    fn collection_face_becomes_a_single_face_file() {
        let mut data = Vec::new();
        data.extend_from_slice(b"ttcf");
        data.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
        data.extend_from_slice(&2_u32.to_be_bytes());
        data.extend_from_slice(&20_u32.to_be_bytes());
        data.extend_from_slice(&48_u32.to_be_bytes());
        let sfnt = |tag: &[u8; 4], table_at: u32| {
            let mut header = Vec::new();
            header.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
            header.extend_from_slice(&1_u16.to_be_bytes());
            header.extend_from_slice(&[0; 6]);
            header.extend_from_slice(tag);
            header.extend_from_slice(&0_u32.to_be_bytes());
            header.extend_from_slice(&table_at.to_be_bytes());
            header.extend_from_slice(&3_u32.to_be_bytes());
            header
        };
        data.extend(sfnt(b"aaaa", 76));
        data.extend(sfnt(b"bbbb", 80));
        data.extend_from_slice(b"AAA\0BBB\0");

        let second = single_face(&data, 1).expect("face 1 extracts");
        assert_eq!(&second[12..16], b"bbbb");
        assert_eq!(u32::from_be_bytes(second[20..24].try_into().unwrap()), 28);
        assert_eq!(&second[28..31], b"BBB");
        assert_eq!(second.len() % 4, 0);
        assert_eq!(single_face(b"OTTO-plain", 0), Some(b"OTTO-plain".to_vec()));
        assert_eq!(single_face(b"OTTO-plain", 1), None);
    }
}
