//! Rows as text: TSV for the clipboard, CSV for a download. Plain text, never
//! markup; the tag ribbon is not a column and never reaches an export.

use super::data::TableData;

#[derive(Clone, Copy)]
enum Format {
    Tsv,
    Csv,
}

impl Format {
    /// Start a line: TSV separates lines, so every one but the first opens
    /// with a newline.
    fn open_line(self, out: &mut String, first: bool) {
        if matches!(self, Format::Tsv) && !first {
            out.push('\n');
        }
    }

    /// End a line: CSV terminates every line with CRLF.
    fn close_line(self, out: &mut String) {
        if matches!(self, Format::Csv) {
            out.push_str("\r\n");
        }
    }

    fn cell(self, out: &mut String, first: bool, text: &str) {
        match self {
            Format::Tsv => {
                if !first {
                    out.push('\t');
                }
                push_tsv(out, text);
            }
            Format::Csv => {
                if !first {
                    out.push(',');
                }
                push_csv(out, text);
            }
        }
    }
}

/// A cell for TSV: tabs and line breaks become spaces.
fn push_tsv(out: &mut String, text: &str) {
    let mut from = 0;
    for (at, byte) in text.bytes().enumerate() {
        if matches!(byte, b'\t' | b'\n' | b'\r') {
            out.push_str(&text[from..at]);
            out.push(' ');
            from = at + 1;
        }
    }
    out.push_str(&text[from..]);
}

/// A cell for CSV: quoted when it holds a comma, quote or line break.
fn push_csv(out: &mut String, text: &str) {
    if !text.bytes().any(|b| matches!(b, b',' | b'"' | b'\n' | b'\r')) {
        out.push_str(text);
        return;
    }
    out.push('"');
    let mut from = 0;
    for (at, byte) in text.bytes().enumerate() {
        if byte == b'"' {
            out.push_str(&text[from..=at]);
            out.push('"');
            from = at + 1;
        }
    }
    out.push_str(&text[from..]);
    out.push('"');
}

fn table<S: AsRef<str>>(format: Format, headers: &[S], rows: &[Vec<S>]) -> String {
    let mut out = String::new();
    format.open_line(&mut out, true);
    for (i, head) in headers.iter().enumerate() {
        format.cell(&mut out, i == 0, head.as_ref());
    }
    format.close_line(&mut out);
    for row in rows {
        format.open_line(&mut out, false);
        for (i, cell) in row.iter().enumerate() {
            format.cell(&mut out, i == 0, cell.as_ref());
        }
        format.close_line(&mut out);
    }
    out
}

fn write_table<S: AsRef<str>>(
    format: Format,
    out: &mut String,
    data: &TableData,
    headers: &[S],
    rows: impl IntoIterator<Item = usize>,
    columns: &[usize],
) {
    let start = out.is_empty();
    format.open_line(out, start);
    for (i, head) in headers.iter().enumerate() {
        format.cell(out, i == 0, head.as_ref());
    }
    format.close_line(out);
    for row in rows {
        format.open_line(out, false);
        for (i, &column) in columns.iter().enumerate() {
            data.with_text(row, column, |text| format.cell(out, i == 0, text));
        }
        format.close_line(out);
    }
}

/// Tab-separated, a header line first; tabs and newlines inside a cell become
/// spaces.
pub fn tsv<S: AsRef<str>>(headers: &[S], rows: &[Vec<S>]) -> String {
    table(Format::Tsv, headers, rows)
}

/// RFC 4180 CSV, a header line first, CRLF after every line; a cell holding a
/// comma, quote or line break is quoted with quotes doubled.
pub fn csv<S: AsRef<str>>(headers: &[S], rows: &[Vec<S>]) -> String {
    table(Format::Csv, headers, rows)
}

/// [`tsv`] of `data`'s `rows` over `columns`, appended to `out` without a
/// `String` per cell.
pub fn write_tsv<S: AsRef<str>>(
    out: &mut String,
    data: &TableData,
    headers: &[S],
    rows: impl IntoIterator<Item = usize>,
    columns: &[usize],
) {
    write_table(Format::Tsv, out, data, headers, rows, columns);
}

/// [`csv`] of `data`'s `rows` over `columns`, appended to `out` without a
/// `String` per cell.
pub fn write_csv<S: AsRef<str>>(
    out: &mut String,
    data: &TableData,
    headers: &[S],
    rows: impl IntoIterator<Item = usize>,
    columns: &[usize],
) {
    write_table(Format::Csv, out, data, headers, rows, columns);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::data_table::{DataColumnSpec, TableDataBuilder, Value};
    use proptest::prelude::*;
    use rstest::rstest;

    /// The allocating implementation the streaming writers replaced.
    fn reference_tsv(headers: &[String], rows: &[Vec<String>]) -> String {
        let line = |cells: &[String]| {
            cells
                .iter()
                .map(|cell| cell.replace(['\t', '\n', '\r'], " "))
                .collect::<Vec<_>>()
                .join("\t")
        };
        std::iter::once(line(headers))
            .chain(rows.iter().map(|row| line(row)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn reference_csv(headers: &[String], rows: &[Vec<String>]) -> String {
        let cell = |text: &str| {
            if text.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", text.replace('"', "\"\""))
            } else {
                text.to_string()
            }
        };
        let line = |cells: &[String]| cells.iter().map(|c| cell(c)).collect::<Vec<_>>().join(",");
        std::iter::once(line(headers))
            .chain(rows.iter().map(|row| line(row)))
            .map(|line| line + "\r\n")
            .collect()
    }

    /// `rows` of text cells as a table, and the same cells as strings.
    fn data_of(rows: &[Vec<String>], width: usize) -> TableData {
        let columns: Vec<DataColumnSpec> = (0..width)
            .map(|c| DataColumnSpec::new(format!("c{c}"), format!("c{c}")))
            .collect();
        let mut builder = TableDataBuilder::with_capacity(&columns, rows.len());
        for (n, row) in rows.iter().enumerate() {
            builder.row(&n.to_string(), row.iter().map(|c| Value::Text(c)));
        }
        builder.build()
    }

    fn streamed(
        write: fn(&mut String, &TableData, &[String], Vec<usize>, &[usize]),
        headers: &[String],
        rows: &[Vec<String>],
    ) -> String {
        let width = headers.len();
        let data = data_of(rows, width);
        let columns: Vec<usize> = (0..width).collect();
        let mut out = String::new();
        write(&mut out, &data, headers, (0..rows.len()).collect(), &columns);
        out
    }

    fn strings(cells: &[&str]) -> Vec<String> {
        cells.iter().map(|c| c.to_string()).collect()
    }

    #[rstest]
    #[case::plain(vec!["a", "b"], vec![vec!["x", "y"]])]
    #[case::quote(vec!["a", "b"], vec![vec!["say \"hi\"", "\""]])]
    #[case::comma(vec!["a,b", "c"], vec![vec!["1,2", ","]])]
    #[case::tab(vec!["a", "b"], vec![vec!["x\ty", "\t"]])]
    #[case::newlines(vec!["a", "b"], vec![vec!["l1\nl2", "\r\n"], vec!["\r", "\n\n"]])]
    #[case::empty_cells(vec!["a", "b"], vec![vec!["", ""], vec!["", "z"]])]
    #[case::no_rows(vec!["a", "b"], vec![])]
    #[case::one_empty_column(vec!["a"], vec![vec![""], vec![""]])]
    #[case::unicode(vec!["é", "日本"], vec![vec!["naïve, \"ü\"", "日\t本"]])]
    fn streaming_writers_match_the_allocating_reference(
        #[case] headers: Vec<&str>,
        #[case] rows: Vec<Vec<&str>>,
    ) {
        let headers = strings(&headers);
        let rows: Vec<Vec<String>> = rows.iter().map(|r| strings(r)).collect();
        assert_eq!(
            streamed(write_tsv, &headers, &rows),
            reference_tsv(&headers, &rows)
        );
        assert_eq!(
            streamed(write_csv, &headers, &rows),
            reference_csv(&headers, &rows)
        );
        assert_eq!(tsv(&headers, &rows), reference_tsv(&headers, &rows));
        assert_eq!(csv(&headers, &rows), reference_csv(&headers, &rows));
    }

    #[test]
    fn numbers_and_enums_export_as_their_text() {
        let columns = [
            DataColumnSpec::new("n", "n").number(),
            DataColumnSpec::new("s", "s").enumerated(),
        ];
        let mut builder = TableDataBuilder::with_capacity(&columns, 3);
        builder.row("a", [Value::Number(3.0), Value::Text("open")]);
        builder.row("b", [Value::Number(2.5), Value::Missing]);
        builder.row("c", [Value::Missing, Value::Text("a,b")]);
        let data = builder.build();
        let mut out = String::new();
        write_csv(&mut out, &data, &["n", "s"], [0, 1, 2], &[0, 1]);
        assert_eq!(out, "n,s\r\n3,open\r\n2.5,\r\n,\"a,b\"\r\n");
        let mut out = String::new();
        write_tsv(&mut out, &data, &["s"], [2, 0], &[1]);
        assert_eq!(out, "s\na,b\nopen");
    }

    #[test]
    fn a_subset_of_rows_and_columns_keeps_the_given_order() {
        let rows: Vec<Vec<String>> = (0..5)
            .map(|r| (0..3).map(|c| format!("{r}{c}")).collect())
            .collect();
        let data = data_of(&rows, 3);
        let mut out = String::new();
        write_csv(&mut out, &data, &["c2", "c0"], [4, 1], &[2, 0]);
        assert_eq!(out, "c2,c0\r\n42,40\r\n12,10\r\n");
    }

    proptest! {
        #[test]
        fn streaming_equals_the_reference_for_arbitrary_cells(
            width in 1usize..4,
            cells in proptest::collection::vec(
                proptest::collection::vec("[a-c ,\"\t\r\né]{0,6}", 3),
                0..8,
            ),
            heads in proptest::collection::vec("[a-c ,\"\t\r\n]{0,4}", 3),
        ) {
            let headers: Vec<String> = heads.into_iter().take(width).collect();
            let rows: Vec<Vec<String>> =
                cells.into_iter().map(|r| r.into_iter().take(width).collect()).collect();
            prop_assert_eq!(
                streamed(write_tsv, &headers, &rows),
                reference_tsv(&headers, &rows)
            );
            prop_assert_eq!(
                streamed(write_csv, &headers, &rows),
                reference_csv(&headers, &rows)
            );
        }
    }
}
