//! OKF engine tests. Inline fixtures cover what the corpus cannot show on its
//! own: resolution of `./`, `../` and bundle-root paths, the reserved
//! filenames, the targets that must produce no morphism, trust derivation,
//! kind mapping and defaulting, list/map field projection, bundle
//! containment, and determinism. The pinned
//! `GoogleCloudPlatform/knowledge-catalog` snapshot, when `PANEL_KIT_OKF_DATA`
//! and `PANEL_KIT_OKF_GRAMMAR` point at it, pins the per-kind counts.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::{CompiledGrammar, EmitterSpec, GrammarError, GrammarPackage, JsonEmitter, Site};

/// An OKF package exercising every rule the engine reads, with the same
/// vocabulary the Nix-produced `grammar-okf` declares.
const OKF_GRAMMAR: &str = r##"{
  "format_version": 1,
  "metadata": {
    "id": "okf-test",
    "name": "OKF",
    "version": "1.0.0",
    "description": "Inline OKF grammar for engine tests."
  },
  "limits": { "grammar_bytes": 8192, "input_bytes": 65536, "nodes": 64, "edges": 256 },
  "parser": {
    "engine": "okf",
    "kinds": {
      "Attested Computation": "computation",
      "Metric": "metric",
      "Policy": "policy"
    },
    "default_kind": "concept",
    "link_rules": {
      "attester.resource": "attested_by",
      "computation": "computes",
      "executor.resource": "executed_by",
      "sources[*].resource": "derived_from"
    },
    "body_links": "references",
    "fields": ["description", "parameters", "status", "stale_after", "usage_window"],
    "bundle_kind": "bundle",
    "contains_kind": "contains"
  }
}"##;

fn grammar() -> CompiledGrammar {
    GrammarPackage::from_json_str(OKF_GRAMMAR).unwrap().compile().unwrap()
}

/// A concept document: frontmatter block plus body.
fn document(frontmatter: &str, body: &str) -> String {
    format!("---\n{frontmatter}---\n{body}")
}

/// The JSON envelope the engine reads, from `(bundle, path, text)` triples in
/// the order the Nix producer emits them.
fn envelope(bundles: &[(&str, &[(&str, &str)])]) -> String {
    let bundles: Vec<Value> = bundles
        .iter()
        .map(|(name, files)| {
            json!({
                "name": name,
                "files": files
                    .iter()
                    .map(|(path, text)| json!({ "path": path, "text": text }))
                    .collect::<Vec<Value>>(),
            })
        })
        .collect();
    json!({ "bundles": bundles }).to_string()
}

/// Morphisms as `(domain, codomain, kind)` triples, in site order.
fn morphisms(site: &Site) -> Vec<(String, String, String)> {
    site.morphisms
        .iter()
        .map(|morphism| {
            (
                morphism.domain.clone(),
                morphism.codomain.clone(),
                morphism.kind.clone().expect("every okf morphism is typed"),
            )
        })
        .collect()
}

/// How many morphisms carry `kind`.
fn morphisms_of(site: &Site, kind: &str) -> usize {
    site.morphisms.iter().filter(|morphism| morphism.kind.as_deref() == Some(kind)).count()
}

#[test]
fn ids_are_bundle_qualified_and_bundles_come_first() {
    let site = grammar()
        .parse(&envelope(&[("b", &[("a.md", &document("type: Metric\n", ""))])]))
        .unwrap();
    let ids: Vec<&str> = site.objects.iter().map(|object| object.id.as_str()).collect();
    assert_eq!(ids, ["b", "b/a"]);
    assert_eq!(site.objects[0].kind, "bundle");
    assert_eq!(site.objects[1].kind, "metric");
}

#[test]
fn two_objects_on_one_id_are_refused() {
    let concept = grammar()
        .parse(&envelope(&[("b", &[("a.md", &document("type: Metric\n", ""))])]))
        .unwrap();
    assert_eq!(concept.objects.len(), 2);

    let repeated_bundle = grammar()
        .parse(&envelope(&[("b", &[]), ("b", &[])]))
        .unwrap_err();
    assert!(matches!(&repeated_bundle, GrammarError::DuplicateNodeId(id) if id == "b"), "{repeated_bundle}");

    // `b/a` is both a bundle's object id and the concept id inside `b`.
    let collision = grammar()
        .parse(&envelope(&[("b", &[("a.md", &document("type: Metric\n", ""))]), ("b/a", &[])]))
        .unwrap_err();
    assert!(matches!(&collision, GrammarError::DuplicateNodeId(id) if id == "b/a"), "{collision}");
}

#[test]
fn a_bundle_contains_each_of_its_concepts() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("a.md", &document("type: Metric\n", "")),
                ("dir/c.md", &document("type: Policy\n", "")),
            ],
        )]))
        .unwrap();
    assert_eq!(
        morphisms(&site),
        [
            ("b".to_owned(), "b/a".to_owned(), "contains".to_owned()),
            ("b".to_owned(), "b/dir/c".to_owned(), "contains".to_owned()),
        ]
    );
}

#[test]
fn document_relative_links_resolve_against_the_linking_file() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("dir/a.md", &document("type: Metric\n", "See [up](../e.md).")),
                ("dir/sub/c.md", &document("type: Metric\n", "")),
                ("e.md", &document("type: Metric\n", "")),
            ],
        )]))
        .unwrap();
    assert!(morphisms(&site).contains(&("b/dir/a".to_owned(), "b/e".to_owned(), "references".to_owned())));
}

#[test]
fn a_dot_slash_link_resolves_next_to_the_linking_file() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("dir/a.md", &document("type: Metric\n", "See [sibling](./sibling.md).")),
                ("dir/sibling.md", &document("type: Metric\n", "")),
            ],
        )]))
        .unwrap();
    assert!(morphisms(&site).contains(&("b/dir/a".to_owned(), "b/dir/sibling".to_owned(), "references".to_owned())));
}

#[test]
fn frontmatter_resources_resolve_against_the_bundle_root() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("policies/p.md", &document("type: Policy\n", "")),
                (
                    "metrics/m.md",
                    &document(
                        concat!(
                            "type: Metric\n",
                            "sources:\n",
                            "  - resource: policies/p.md\n",
                            "  - resource: https://example.com/policy\n",
                        ),
                        "",
                    ),
                ),
            ],
        )]))
        .unwrap();
    assert_eq!(
        morphisms_of(&site, "derived_from"),
        1,
        "the external source must not become a morphism: {:#?}",
        morphisms(&site)
    );
    assert!(morphisms(&site).contains(&("b/metrics/m".to_owned(), "b/policies/p".to_owned(), "derived_from".to_owned())));
}

#[test]
fn each_link_rule_contributes_its_own_kind() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("skills/s.md", &document("type: Policy\n", "")),
                ("attesters/a.py", ""),
                ("c.md", &document("type: Metric\n", "")),
                (
                    "run.md",
                    &document(
                        concat!(
                            "type: Attested Computation\n",
                            "computation: c.md\n",
                            "executor:\n",
                            "  resource: skills/s.md\n",
                            "attester:\n",
                            "  resource: attesters/a.py\n",
                        ),
                        "",
                    ),
                ),
            ],
        )]))
        .unwrap();
    assert!(morphisms(&site).contains(&("b/run".to_owned(), "b/c".to_owned(), "computes".to_owned())));
    assert!(morphisms(&site).contains(&("b/run".to_owned(), "b/skills/s".to_owned(), "executed_by".to_owned())));
    assert_eq!(morphisms_of(&site, "attested_by"), 0, "a .py target is not a concept");
}

#[test]
fn reserved_files_are_not_concepts() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("index.md", &document("type: Metric\n", "")),
                ("log.md", &document("type: Metric\n", "")),
                ("dir/index.md", &document("type: Metric\n", "")),
                ("a.md", &document("type: Metric\n", "")),
            ],
        )]))
        .unwrap();
    let ids: Vec<&str> = site.objects.iter().map(|object| object.id.as_str()).collect();
    assert_eq!(ids, ["b", "b/a"]);
    assert_eq!(morphisms_of(&site, "contains"), 1);
}

#[test]
fn external_image_self_missing_and_non_md_targets_make_no_morphism() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("b.md", &document("type: Metric\n", "")),
                ("data.csv", ""),
                (
                    "a.md",
                    &document(
                        "type: Metric\n",
                        concat!(
                            "[out](https://example.com/x.md)\n",
                            "[proto](//example.com/x.md)\n",
                            "![alt](chart.png)\n",
                            "[self](./a.md)\n",
                            "[gone](./nowhere.md)\n",
                            "[csv](data.csv)\n",
                            "[fragment](b.md#section)\n",
                            "[query](b.md?v=1)\n",
                            "[escapes](../../../outside.md)\n",
                            "[badge](https://img.shields.io/badge/x.svg)\n",
                        ),
                    ),
                ),
            ],
        )]))
        .unwrap();
    assert_eq!(morphisms_of(&site, "references"), 1, "only the fragment link resolves: {:#?}", morphisms(&site));
    assert!(morphisms(&site).contains(&("b/a".to_owned(), "b/b".to_owned(), "references".to_owned())));
}

#[test]
fn a_link_whose_text_is_an_image_is_prose_not_a_reference() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("b.md", &document("type: Metric\n", "")),
                ("a.md", &document("type: Metric\n", "[![badge](chart.png)](b.md)\n")),
            ],
        )]))
        .unwrap();
    assert_eq!(morphisms_of(&site, "references"), 0);
}

#[test]
fn unresolved_links_are_counted_not_fatal() {
    let report = grammar()
        .parse_reported(&envelope(&[(
            "b",
            &[
                ("a.md", &document("type: Metric\n", "[x](./gone.md) [y](also-gone.md)\n")),
            ],
        )]))
        .unwrap();
    assert_eq!(report.unresolved_links, 2);
    assert_eq!(morphisms_of(&report.site, "references"), 0);
}

#[test]
fn unresolved_links_from_frontmatter_are_counted_too() {
    let report = grammar()
        .parse_reported(&envelope(&[(
            "b",
            &[(
                "a.md",
                &document("type: Metric\nsources:\n  - resource: missing.md\n", ""),
            )],
        )]))
        .unwrap();
    assert_eq!(report.unresolved_links, 1);
}

#[test]
fn a_self_link_is_resolved_so_it_is_not_unresolved() {
    let report = grammar()
        .parse_reported(&envelope(&[(
            "b",
            &[("a.md", &document("type: Metric\n", "[self](./a.md)\n"))],
        )]))
        .unwrap();
    assert_eq!(report.unresolved_links, 0);
    assert_eq!(morphisms_of(&report.site, "references"), 0);
}

#[test]
fn the_trust_tier_follows_verified_then_generated() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("human.md", &document("type: Metric\nverified:\n  - { by: \"human:kim\", at: 2026-01-01T00:00:00Z }\n", "")),
                ("bare-human.md", &document("type: Metric\nverified: { by: \"human:kim\" }\n", "")),
                ("machine.md", &document("type: Metric\nverified:\n  - { by: \"process:nightly\" }\n", "")),
                ("generated.md", &document("type: Metric\ngenerated: { by: agent/1 }\n", "")),
                ("bare.md", &document("type: Metric\n", "")),
            ],
        )]))
        .unwrap();
    let trust = |id: &str| {
        site.objects
            .iter()
            .find(|object| object.id == id)
            .unwrap()
            .fields
            .get("trust")
            .cloned()
            .unwrap_or_default()
    };
    assert_eq!(trust("b/human"), "human-reviewed");
    assert_eq!(trust("b/bare-human"), "human-reviewed", "a bare mapping counts as a one-element list");
    assert_eq!(trust("b/machine"), "unattested", "only `generated` sets the machine tier");
    assert_eq!(trust("b/generated"), "machine-generated");
    assert_eq!(trust("b/bare"), "unattested");
}

#[test]
fn unmapped_types_take_the_default_kind() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[("a.md", &document("type: Something New\n", ""))],
        )]))
        .unwrap();
    assert_eq!(site.objects[1].kind, "concept");
}

#[test]
fn fields_project_scalars_directly_and_structures_as_compact_json() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[(
                "a.md",
                &document(
                    concat!(
                        "type: Metric\n",
                        "description: A metric\n",
                        "stale_after: 2026-12-31T00:00:00Z\n",
                        "status: stable\n",
                        "usage_window: { from: 2026-01-01, to: 2026-06-30 }\n",
                        "parameters:\n",
                        "  - { name: period_start, type: date }\n",
                        "  - { name: period_end, type: date }\n",
                    ),
                    "",
                ),
            )],
        )]))
        .unwrap();
    let fields = &site.objects[1].fields;
    assert_eq!(fields["description"], "A metric");
    assert_eq!(fields["stale_after"], "2026-12-31T00:00:00Z");
    assert_eq!(fields["status"], "stable");
    assert_eq!(
        fields["usage_window"],
        r#"{"from":"2026-01-01","to":"2026-06-30"}"#,
        "a map projects as compact JSON"
    );
    assert_eq!(
        fields["parameters"],
        r#"[{"name":"period_start","type":"date"},{"name":"period_end","type":"date"}]"#,
        "a list projects as compact JSON"
    );
    assert_eq!(fields["trust"], "unattested", "the derived tier is always present");
}

#[test]
fn titles_fall_back_to_the_file_stem_and_tags_are_sorted_and_deduplicated() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("dir/untitled.md", &document("type: Metric\n", "")),
                (
                    "b.md",
                    &document("type: Metric\ntitle: Named\ntags: [zeta, alpha, zeta]\n", ""),
                ),
            ],
        )]))
        .unwrap();
    let titled = |id: &str| site.objects.iter().find(|object| object.id == id).unwrap();
    assert_eq!(titled("b/dir/untitled").title, "untitled");
    assert_eq!(titled("b/b").title, "Named");
    assert_eq!(titled("b/b").tags, ["alpha", "zeta"]);
    assert!(titled("b/dir/untitled").tags.is_empty());
}

#[test]
fn parsing_twice_yields_an_equal_site() {
    let input = envelope(&[
        (
            "b",
            &[
                ("a.md", &document("type: Metric\n", "[c](c.md)")),
                ("c.md", &document("type: Policy\n", "[a](./a.md)")),
            ],
        ),
        ("other", &[("z.md", &document("type: Attested Computation\n", ""))]),
    ]);
    let first = grammar().parse(&input).unwrap();
    let second = grammar().parse(&input).unwrap();
    assert_eq!(first, second);
}

#[test]
fn duplicate_morphisms_collapse_per_domain_codomain_and_kind() {
    let site = grammar()
        .parse(&envelope(&[(
            "b",
            &[
                ("a.md", &document("type: Metric\n", "[c](c.md) [again](c.md)\n")),
                ("c.md", &document("type: Metric\n", "")),
            ],
        )]))
        .unwrap();
    assert_eq!(morphisms_of(&site, "references"), 1);
}

#[test]
fn a_concept_without_a_type_is_a_parse_error() {
    let error = grammar()
        .parse(&envelope(&[("b", &[("a.md", &document("title: No type\n", ""))])]))
        .unwrap_err();
    assert!(
        matches!(&error, GrammarError::InvalidRecord { record: "concept", detail } if detail.contains("requires a non-empty string `type`")),
        "{error}"
    );
}

#[test]
fn a_status_outside_the_lifecycle_is_a_parse_error() {
    let error = grammar()
        .parse(&envelope(&[("b", &[("a.md", &document("type: Metric\nstatus: retired\n", ""))])]))
        .unwrap_err();
    assert!(
        matches!(&error, GrammarError::InvalidRecord { record: "concept", detail } if detail.contains("draft, stable, or deprecated")),
        "{error}"
    );
}

#[test]
fn a_document_without_frontmatter_is_a_parse_error() {
    let error = grammar().parse(&envelope(&[("b", &[("a.md", "no frontmatter here\n")])])).unwrap_err();
    assert!(matches!(error, GrammarError::InvalidRecord { record: "concept", .. }), "{error}");
}

#[test]
fn malformed_tags_are_rejected_rather_than_silently_dropped() {
    let error = grammar()
        .parse(&envelope(&[("b", &[("a.md", &document("type: Metric\ntags: nope\n", ""))])]))
        .unwrap_err();
    assert!(matches!(error, GrammarError::InvalidRecord { record: "concept", .. }), "{error}");
}

#[test]
fn a_rule_table_over_the_grammar_byte_limit_is_refused() {
    let mut package = GrammarPackage::from_json_str(OKF_GRAMMAR).unwrap();
    package.limits.grammar_bytes = 16;
    assert!(matches!(package.compile(), Err(GrammarError::GrammarTooLarge { .. })));
}

#[test]
fn the_node_ceiling_counts_bundles_and_concepts() {
    let mut package = GrammarPackage::from_json_str(OKF_GRAMMAR).unwrap();
    package.limits.nodes = 2;
    let compiled = package.compile().unwrap();
    let input = envelope(&[(
        "b",
        &[
            ("a.md", &document("type: Metric\n", "")),
            ("c.md", &document("type: Metric\n", "")),
        ],
    )]);
    assert!(
        matches!(compiled.parse(&input), Err(GrammarError::RecordLimit { kind: "nodes", actual: 3, max: 2 })),
        "one bundle plus two concepts is three nodes"
    );
}

#[test]
fn the_morphism_ceiling_is_enforced_rather_than_truncating() {
    let mut package = GrammarPackage::from_json_str(OKF_GRAMMAR).unwrap();
    package.limits.edges = 1;
    let compiled = package.compile().unwrap();
    let input = envelope(&[(
        "b",
        &[
            ("a.md", &document("type: Metric\n", "")),
            ("c.md", &document("type: Metric\n", "")),
            ("d.md", &document("type: Metric\n", "")),
        ],
    )]);
    assert!(matches!(
        compiled.parse(&input),
        Err(GrammarError::RecordLimit { kind: "edges", max: 1, .. })
    ));
}

#[test]
fn an_emitter_for_another_engine_is_refused() {
    let mut package = GrammarPackage::from_json_str(OKF_GRAMMAR).unwrap();
    package.emitter = Some(EmitterSpec::Json(JsonEmitter {}));
    assert!(
        matches!(package.compile(), Err(GrammarError::EmitterEngine { parser: "okf", emitter: "json" })),
    );
}

#[test]
fn an_okf_package_declares_no_emitter() {
    let site = Site { objects: Vec::new(), morphisms: Vec::new() };
    let error = grammar().emit(&site, &BTreeSet::new()).expect_err("okf declares no emitter");
    assert!(matches!(error, GrammarError::NoEmitter), "{error}");
}

// --- The pinned GoogleCloudPlatform/knowledge-catalog snapshot -------------

/// The snapshot envelope and the package `nix/grammars/okf.nix` produces, as
/// built by `nix build .#okf-knowledge-catalog .#grammar-okf`.
fn snapshot() -> Option<(CompiledGrammar, String)> {
    let data = option_env!("PANEL_KIT_OKF_DATA")?;
    let package = option_env!("PANEL_KIT_OKF_GRAMMAR")?;
    let source = std::fs::read_to_string(package).expect("PANEL_KIT_OKF_GRAMMAR must be readable");
    let compiled = GrammarPackage::from_json_str(&source)
        .expect("PANEL_KIT_OKF_GRAMMAR must be a GrammarPackage")
        .compile()
        .expect("the okf grammar package must compile");
    let envelope = std::fs::read_to_string(data).expect("PANEL_KIT_OKF_DATA must be readable");
    Some((compiled, envelope))
}

#[test]
fn the_pinned_snapshot_maps_to_the_documented_site() {
    let Some((compiled, envelope)) = snapshot() else {
        eprintln!("skipping: set PANEL_KIT_OKF_DATA and PANEL_KIT_OKF_GRAMMAR to canary the snapshot");
        return;
    };
    let report = compiled.parse_reported(&envelope).unwrap();
    let site = &report.site;

    let objects: BTreeMap<&str, usize> = tally(site.objects.iter().map(|object| object.kind.as_str()));
    assert_eq!(
        objects,
        BTreeMap::from([
            ("bundle", 4),
            ("computation", 2),
            ("dataset", 3),
            ("metric", 3),
            ("policy", 2),
            ("reference", 20),
            ("skill", 1),
            ("table", 22),
        ])
    );
    let morphisms: BTreeMap<&str, usize> =
        tally(site.morphisms.iter().map(|morphism| morphism.kind.as_deref().unwrap_or("")));
    assert_eq!(
        morphisms,
        BTreeMap::from([
            ("contains", 53),
            ("derived_from", 8),
            ("executed_by", 2),
            ("references", 97),
        ])
    );
    assert_eq!(report.unresolved_links, 0, "every internal reference in the pinned corpus resolves");
    // Every `attester.resource` in the corpus names a `.py`, and no concept
    // declares `computation`, so neither rule fires here.
    assert_eq!(morphisms_of(site, "attested_by"), 0);
    assert_eq!(morphisms_of(site, "computes"), 0);

    // The corpus' 78 documents minus its 25 reserved files (`index.md` at
    // any depth, plus `acme_retail/log.md`) are 53 concepts, and the four
    // bundles carry a `contains` morphism to each of them.
    let concepts: Vec<&str> =
        site.objects.iter().filter(|object| object.kind != "bundle").map(|object| object.id.as_str()).collect();
    let contained: BTreeSet<(&str, &str)> = site
        .morphisms
        .iter()
        .filter(|morphism| morphism.kind.as_deref() == Some("contains"))
        .map(|morphism| (morphism.domain.as_str(), morphism.codomain.as_str()))
        .collect();
    assert_eq!(concepts.len(), 53);
    for id in &concepts {
        let bundle = id.split_once('/').expect("a concept id is bundle-qualified").0;
        assert!(contained.contains(&(bundle, id)), "{id} is not contained by {bundle}");
    }
    assert_eq!(contained.len(), concepts.len());

    let ids: BTreeSet<&str> = site.objects.iter().map(|object| object.id.as_str()).collect();
    for morphism in &site.morphisms {
        assert!(ids.contains(morphism.domain.as_str()), "dangling domain {}", morphism.domain);
        assert!(ids.contains(morphism.codomain.as_str()), "dangling codomain {}", morphism.codomain);
        assert!(morphism.kind.is_some(), "every okf morphism is typed");
    }
 
    // Bundles lead, in envelope (sorted name) order, then concepts.
    assert_eq!(
        site.objects[..4].iter().map(|object| object.id.as_str()).collect::<Vec<_>>(),
        ["acme_retail", "crypto_bitcoin", "ga4", "stackoverflow"]
    );
    let computation = site.objects.iter().find(|object| object.id == "acme_retail/computations/gross-margin-period").unwrap();
    assert_eq!(computation.kind, "computation");
    assert_eq!(computation.fields["trust"], "human-reviewed");
    assert_eq!(computation.tags, ["attested", "finance", "margin"]);
}

#[test]
fn the_pinned_snapshot_parses_to_an_equal_site_twice() {
    let Some((compiled, envelope)) = snapshot() else {
        eprintln!("skipping: set PANEL_KIT_OKF_DATA and PANEL_KIT_OKF_GRAMMAR to canary the snapshot");
        return;
    };
    assert_eq!(compiled.parse(&envelope).unwrap(), compiled.parse(&envelope).unwrap());
}

fn tally<'a>(values: impl Iterator<Item = &'a str>) -> BTreeMap<&'a str, usize> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
}
