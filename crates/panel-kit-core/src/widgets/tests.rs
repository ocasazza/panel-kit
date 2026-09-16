use super::*;

#[test]
fn content_spec_kind_names_are_derived_from_variants() {
    let specs = all_content_spec_variants();
    let observed: Vec<_> = specs
        .iter()
        .map(|spec| {
            let encoded = serde_json::to_value(spec).expect("content spec serializes");
            assert_eq!(encoded["kind"], spec.kind());
            spec.kind()
        })
        .collect();

    assert_eq!(observed.len(), 12);
    assert_eq!(
        observed,
        [
            "custom",
            "text",
            "editor",
            "badges",
            "table",
            "time_series",
            "gauges",
            "flamegraph",
            "boxplot",
            "meter",
            "status",
            "spinner",
        ]
    );
    assert!(!observed.contains(&"unsupported"));
}

#[test]
fn runtime_content_views_borrow_their_buffers() {
    let badges = [crate::badge::BadgeSpec::new(
        "tag",
        "alpha",
        crate::badge::BadgeKind::Tag,
    )];
    let view = ContentView::Badges(&badges);

    assert_eq!(view.kind(), "badges");
    assert_eq!(view.is_empty(), Some(false));
}

#[test]
fn data_source_preserves_inline_and_binding_sources() {
    let inline = DataSource::Inline {
        value: "hello".to_string(),
    };
    let binding: DataSource<String> = DataSource::Binding {
        id: "notes.body".into(),
    };

    assert!(inline.is_inline());
    assert_eq!(binding.binding_id(), Some("notes.body"));
}

fn all_content_spec_variants() -> [ContentSpec; 12] {
    [
        ContentSpec::Custom {
            binding: "native.panel".into(),
        },
        ContentSpec::Text {
            source: DataSource::Inline {
                value: TextModel {
                    text: "hello".into(),
                },
            },
            scroll: ScrollPolicy::Wrap,
        },
        ContentSpec::Editor {
            binding: "editor.body".into(),
            multiline: true,
            placeholder: "type here".into(),
        },
        ContentSpec::Badges {
            source: DataSource::Inline {
                value: vec![crate::badge::BadgeSpec::new(
                    "tag",
                    "alpha",
                    crate::badge::BadgeKind::Tag,
                )],
            },
        },
        ContentSpec::Table {
            source: DataSource::Inline {
                value: table::TableModel {
                    columns: vec![table::TableColumn {
                        key: "name".into(),
                        title: "Name".into(),
                        width: table::ColumnWidth::Flex { weight: 1 },
                        align: table::TextAlign::Left,
                    }],
                    rows: vec![table::TableRow {
                        cells: vec![table::TableCell::Text("api".into())],
                    }],
                },
            },
        },
        ContentSpec::TimeSeries {
            source: DataSource::Inline {
                value: vec![charts::SeriesModel {
                    name: "latency".into(),
                    points: vec![(0.0, 1.0)],
                }],
            },
            unit: "ms".into(),
        },
        ContentSpec::Gauges {
            source: DataSource::Inline {
                value: vec![charts::GaugeModel {
                    label: "queue".into(),
                    ratio: 0.5,
                    text: "50%".into(),
                }],
            },
        },
        ContentSpec::Flamegraph {
            source: DataSource::Inline {
                value: vec![charts::FlameSpanModel {
                    label: "root".into(),
                    depth: 0,
                    value: 1.0,
                    color: None,
                }],
            },
        },
        ContentSpec::Boxplot {
            source: DataSource::Inline {
                value: vec![charts::BoxItemModel {
                    label: "stage".into(),
                    samples: vec![1.0, 2.0, 3.0],
                    color: None,
                }],
            },
        },
        ContentSpec::Meter {
            source: DataSource::Inline {
                value: meter::MeterModel {
                    label: "cpu".into(),
                    ratio: 0.42,
                    text: "42%".into(),
                    color: None,
                },
            },
        },
        ContentSpec::Status {
            source: DataSource::Inline {
                value: status::StatusModel {
                    label: "build".into(),
                    state: status::StatusState::Ok,
                    color: (0, 255, 0),
                },
            },
        },
        ContentSpec::Spinner {
            label: DataSource::Inline {
                value: "loading".into(),
            },
        },
    ]
}
