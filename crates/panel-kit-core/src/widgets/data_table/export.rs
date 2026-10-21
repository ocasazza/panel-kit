//! Rows as text: TSV for the clipboard, CSV for a download. Plain text, never
//! markup; the tag ribbon is not a column and never reaches an export.

/// Tab-separated, a header line first; tabs and newlines inside a cell become
/// spaces.
pub fn tsv<S: AsRef<str>>(headers: &[S], rows: &[Vec<S>]) -> String {
    let line = |cells: &[S]| {
        cells
            .iter()
            .map(|cell| cell.as_ref().replace(['\t', '\n', '\r'], " "))
            .collect::<Vec<_>>()
            .join("\t")
    };
    std::iter::once(line(headers))
        .chain(rows.iter().map(|row| line(row)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// RFC 4180 CSV, a header line first, CRLF after every line; a cell holding a
/// comma, quote or line break is quoted with quotes doubled.
pub fn csv<S: AsRef<str>>(headers: &[S], rows: &[Vec<S>]) -> String {
    let cell = |text: &str| {
        if text.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", text.replace('"', "\"\""))
        } else {
            text.to_string()
        }
    };
    let line = |cells: &[S]| {
        cells
            .iter()
            .map(|c| cell(c.as_ref()))
            .collect::<Vec<_>>()
            .join(",")
    };
    std::iter::once(line(headers))
        .chain(rows.iter().map(|row| line(row)))
        .map(|line| line + "\r\n")
        .collect()
}
