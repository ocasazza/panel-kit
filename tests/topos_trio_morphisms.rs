//! Host-contract test for the trio's morphism diagram, over the real Nix JSON
//! (the `topos-trio-morphisms-test` check exports the topos env, the OKF env and
//! the four morphism documents): ψ: okf-catalog → membrane, χ: okf-catalog →
//! agentic, and ω: agentic → okf-catalog each validate against their domain and
//! codomain, each commutes over the physics base (`p_membrane ∘ ψ =
//! p_agentic ∘ χ = p_catalog ∘ ω = p_catalog`), and the diagram commutes —
//! `f ∘ χ = ψ` on every object and morphism sort, and `ψ ∘ ω = f`.
//!
//! The laws discriminate rather than hold vacuously: ω mapping `reviewer` to
//! `skill` breaks `ψ ∘ ω = f` and is rejected, and so is a ψ whose `concept`
//! sort no longer commutes with the base.
//!
//! Paths come from the environment at run time, so this target compiles under
//! every check.

use panel_kit_grammar::{BaseTopos, GeometricMorphism, Topos};

/// Read a spec named by the check's environment.
fn read(var: &str) -> String {
    let path = std::env::var(var)
        .unwrap_or_else(|_| panic!("{var} is set by the topos-trio-morphisms-test check"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{var}={path}: {error}"))
}

fn topos(var: &str) -> Topos {
    let topos = Topos::from_json_str(&read(var)).expect("topos decodes");
    topos.validate().expect("topos validates");
    topos
}

fn morphism(var: &str) -> GeometricMorphism {
    GeometricMorphism::from_json_str(&read(var)).expect("morphism decodes")
}

/// The `okf-catalog` topos: ψ and χ's common domain.
fn catalog() -> Topos {
    topos("PANEL_KIT_OKF_CATALOG")
}

/// ψ: `okf-catalog → membrane`.
fn psi() -> GeometricMorphism {
    morphism("PANEL_KIT_TRIO_MORPHISM_MEMBRANE")
}

/// χ: `okf-catalog → agentic`.
fn chi() -> GeometricMorphism {
    morphism("PANEL_KIT_TRIO_MORPHISM_AGENTIC")
}

/// ω: `agentic → okf-catalog`.
fn omega() -> GeometricMorphism {
    morphism("PANEL_KIT_TRIO_MORPHISM_OMEGA")
}

/// f: `agentic → membrane`.
fn f() -> GeometricMorphism {
    morphism("PANEL_KIT_MORPHISM")
}

#[test]
fn every_morphism_validates_on_its_domain_and_codomain() {
    let catalog = catalog();
    let agentic = topos("PANEL_KIT_TOPOS_A");
    let membrane = topos("PANEL_KIT_TOPOS_B");

    psi()
        .validate(&catalog, &membrane)
        .expect("psi is total on the catalog sorts and lands in the membrane sorts");
    chi()
        .validate(&catalog, &agentic)
        .expect("chi is total on the catalog sorts and lands in the agentic sorts");
    omega()
        .validate(&agentic, &catalog)
        .expect("omega is total on the agentic sorts and lands in the catalog sorts");
}

#[test]
fn every_morphism_commutes_over_the_physics_base() {
    let p_catalog = morphism("PANEL_KIT_OKF_CATALOG_PHYSICS");
    let p_membrane = morphism("PANEL_KIT_MORPHISM_B_PHYSICS");
    let p_agentic = morphism("PANEL_KIT_MORPHISM_A_PHYSICS");

    psi().validate_over(&p_catalog, &p_membrane)
        .expect("p_membrane . psi = p_catalog on every sort");
    chi().validate_over(&p_catalog, &p_agentic)
        .expect("p_agentic . chi = p_catalog on every sort");
    omega()
        .validate_over(&p_agentic, &p_catalog)
        .expect("p_catalog . omega = p_agentic on every sort");

    // Pulled back along each morphism the base sorts are the catalog's own, so
    // all three regimes induce one physics section over the one base.
    assert_eq!(psi().inverse_image(&p_membrane.object_sorts), p_catalog.object_sorts);
    assert_eq!(chi().inverse_image(&p_agentic.object_sorts), p_catalog.object_sorts);
    assert_eq!(
        omega().inverse_image(&p_catalog.object_sorts),
        p_agentic.object_sorts,
        "a policy reads the agentic engine a planner reads"
    );

    let base =
        BaseTopos::from_json_str(&read("PANEL_KIT_TOPOS_PHYSICS")).expect("the base decodes");
    assert_eq!(base.id, p_catalog.codomain, "p_catalog lands in the one base");
    assert_eq!(base.stalks.objects.len(), 3, "the base has three engine particles");
    assert_eq!(base.stalks.morphisms.len(), 3, "the base has three engine bonds");
}

#[test]
fn the_triangle_commutes_f_after_chi_is_psi() {
    let psi = psi();
    let chi = chi();
    let composed = chi
        .compose(&f())
        .expect("chi: catalog -> agentic composes with f: agentic -> membrane");

    assert_eq!(composed.domain, psi.domain);
    assert_eq!(composed.codomain, psi.codomain);
    assert_eq!(composed.object_sorts, psi.object_sorts, "f . chi = psi on objects");
    assert_eq!(composed.morphism_sorts, psi.morphism_sorts, "f . chi = psi on morphisms");

    // Every catalog sort is covered on both sides: no sort escapes the law.
    let catalog = catalog();
    for sort in catalog.stalks.objects.keys() {
        assert!(psi.object_sorts.contains_key(sort), "{sort} missing from psi");
        assert!(composed.object_sorts.contains_key(sort), "{sort} missing from f . chi");
    }
    for sort in catalog.stalks.morphisms.keys() {
        assert!(psi.morphism_sorts.contains_key(sort), "{sort} missing from psi");
        assert!(composed.morphism_sorts.contains_key(sort), "{sort} missing from f . chi");
    }
}

#[test]
fn the_square_commutes_psi_after_omega_is_f() {
    let f = f();
    let psi = psi();
    let composed = omega()
        .compose(&psi)
        .expect("omega: agentic -> catalog composes with psi: catalog -> membrane");

    assert_eq!(composed.domain, f.domain);
    assert_eq!(composed.codomain, f.codomain);
    assert_eq!(composed.object_sorts, f.object_sorts, "psi . omega = f on objects");
    assert_eq!(composed.morphism_sorts, f.morphism_sorts, "psi . omega = f on morphisms");

    for sort in topos("PANEL_KIT_TOPOS_A").stalks.objects.keys() {
        assert!(composed.object_sorts.contains_key(sort), "{sort} missing from psi . omega");
    }
    for sort in topos("PANEL_KIT_TOPOS_A").stalks.morphisms.keys() {
        assert!(composed.morphism_sorts.contains_key(sort), "{sort} missing from psi . omega");
    }
}

#[test]
fn omega_is_the_map_the_diagram_pins() {
    let omega = omega();
    assert_eq!(omega.domain, "topos-agentic");
    assert_eq!(omega.codomain, "topos-okf-catalog");
    for (sort, image) in [
        ("planner", "policy"),
        ("reviewer", "policy"),
        ("coder", "computation"),
        ("memory", "concept"),
    ] {
        assert_eq!(omega.object_sorts.get(sort).map(String::as_str), Some(image), "omega {sort}");
    }
    for (sort, image) in [
        ("continues", "computes"),
        ("shares", "executed_by"),
        ("consolidates", "references"),
    ] {
        assert_eq!(omega.morphism_sorts.get(sort).map(String::as_str), Some(image), "omega {sort}");
    }
    // Every site kind the agentic regime sorts as one sort lands on that one
    // catalog sort, so the cross morphisms, which are agentic shares, are the
    // catalog's executed_by.
    for sort in ["produces", "verifies"] {
        assert_eq!(omega.morphism_sorts.get("shares").map(String::as_str), Some("executed_by"), "omega shares");
        assert_eq!(catalog().morphism_sorting.get(sort).map(String::as_str), Some("executed_by"), "the catalog sorts {sort}");
    }

    // The data moves with the sorts: a session's energy is the catalog's trust,
    // its latency the catalog's staleness horizon.
    assert_eq!(omega.attribute("trust"), "energy");
    assert_eq!(omega.attribute("stale_after"), "latency_ms");
    assert_eq!(omega.attribute("status"), "status", "an unnamed field is the identity");
    // And the other way round, over a knowledge object.
    assert_eq!(chi().attribute("latency_ms"), "stale_after");
    assert_eq!(chi().attribute("energy"), "stale_after");
    assert_eq!(chi().attribute("status"), "status");
}

#[test]
fn an_omega_that_maps_the_reviewer_to_a_skill_fails_the_square() {
    let p_catalog = morphism("PANEL_KIT_OKF_CATALOG_PHYSICS");
    let p_agentic = morphism("PANEL_KIT_MORPHISM_A_PHYSICS");

    // The real ω commutes, on both laws.
    let square = omega().compose(&psi()).expect("omega then psi composes");
    assert_eq!(square.object_sorts, f().object_sorts, "the authored ψ ∘ ω = f");
    assert!(
        omega().validate_over(&p_agentic, &p_catalog).is_ok(),
        "and it commutes over the base"
    );

    // One object sort remapped: ω stays total into the catalog sorts and still
    // agrees with the catalog's own sorting of `reviewer` is broken too, but the
    // square is the first law it breaks.
    let mut broken = omega();
    broken.object_sorts.insert("reviewer".to_owned(), "skill".to_owned());

    let composed = broken
        .compose(&psi())
        .expect("the square still composes");
    let real = omega().compose(&psi()).expect("the real square composes");
    assert_eq!(real.object_sorts, f().object_sorts, "the real omega commutes");
    assert_ne!(
        composed.object_sorts, broken.object_sorts,
        "one remapped sort breaks psi . omega = f"
    );
    assert_eq!(
        composed.object_sorts.get("reviewer").map(String::as_str),
        Some("tail"),
        "reviewer then psi lands on tail, f lands it on head"
    );
    assert!(
        broken.validate(&topos("PANEL_KIT_TOPOS_A"), &catalog()).is_err(),
        "and the catalog's own sorting of reviewer no longer agrees with omega"
    );
    assert!(
        broken.validate_over(&p_agentic, &p_catalog).is_err(),
        "a reviewer that is a skill also stops commuting with the base"
    );
}

#[test]
fn a_broken_psi_fails_the_commutation_check() {
    let p_catalog = morphism("PANEL_KIT_OKF_CATALOG_PHYSICS");
    let p_membrane = morphism("PANEL_KIT_MORPHISM_B_PHYSICS");

    // One object sort remapped: ψ still total into the membrane sorts, but no
    // longer f . chi on that sort nor p_membrane-commuting over the base.
    let mut broken = psi();
    broken.object_sorts.insert("concept".to_owned(), "tail".to_owned());

    let composed = chi()
        .compose(&f())
        .expect("the triangle still composes");
    assert_eq!(composed.object_sorts, psi().object_sorts, "the real psi commutes");
    assert_ne!(
        composed.object_sorts, broken.object_sorts,
        "one remapped sort breaks f . chi = psi"
    );
    assert!(
        broken.validate_over(&p_catalog, &p_membrane).is_err(),
        "one remapped sort also breaks p_membrane . psi = p_catalog"
    );
    assert!(
        broken.validate_over(&p_catalog, &morphism("PANEL_KIT_MORPHISM_A_PHYSICS")).is_err(),
        "the broken psi is discriminated, not merely different"
    );
}

#[test]
fn the_three_regimes_declare_the_graph_component_and_external_panel() {
    for (var, binding) in [
        ("PANEL_KIT_TOPOS_A", "agentic.graph"),
        ("PANEL_KIT_TOPOS_B", "membrane.graph"),
        ("PANEL_KIT_OKF_CATALOG", "okf.graph"),
    ] {
        // `Topos::validate` above already accepted the host-overlay binding.
        let topos = topos(var);
        let component = topos
            .workspace
            .components
            .iter()
            .find(|component| component.id == "graph")
            .unwrap_or_else(|| panic!("{} declares the graph component", topos.id));
        assert!(component.keep_mounted, "an imperative canvas stays mounted");
        assert!(component.min_size[0] > 0.0 && component.min_size[1] > 0.0);
        assert_eq!(component.events.len(), 2, "select and hover");

        let panel = topos
            .workspace
            .panels
            .iter()
            .find(|panel| panel.content.binding_id() == Some(binding))
            .unwrap_or_else(|| panic!("{} mounts {binding}", topos.id));
        assert_eq!(panel.external_component(), Some("graph"));
        assert_eq!(panel.window.tile_w, 4, "the graph spans the full shelf width");
    }
}

#[test]
fn every_regime_sorts_every_site_kind_of_the_merged_site() {
    // Sessions are native in agentic; knowledge objects are native in
    // okf-catalog; neither is native in membrane, so both are read there by
    // transport along f and psi respectively.
    let agentic = topos("PANEL_KIT_TOPOS_A");
    let catalog = topos("PANEL_KIT_OKF_CATALOG");
    assert_eq!(
        agentic.native_object_kinds(),
        ["coder", "memory", "planner", "reviewer"].into_iter().map(String::from).collect()
    );
    assert_eq!(
        catalog.native_object_kinds(),
        [
            "bundle",
            "computation",
            "concept",
            "dataset",
            "log",
            "metric",
            "policy",
            "reference",
            "skill",
            "table",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );
    let membrane = topos("PANEL_KIT_TOPOS_B");
    assert!(membrane.native_object_kinds().is_empty(), "membrane sorts every kind by transport");

    // ω is checked against the catalog's own sorting of the kinds the agentic
    // regime sorts natively: every other kind the catalog sorts itself.
    let omega = omega();
    for sort in agentic.native_object_kinds() {
        let native_sort = &agentic.object_sorting[&sort];
        assert_eq!(
            catalog.object_sorting.get(&sort),
            omega.object_sorts.get(native_sort),
            "the catalog sorts the native kind {sort} as omega sends it"
        );
    }
}