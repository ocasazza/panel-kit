//! Host-contract test for the trio's morphism triangle, over the real Nix JSON
//! (the `topos-trio-morphisms-test` check exports the topos env, the OKF env and
//! the two new morphism documents): ψ: okf-catalog → membrane and
//! χ: okf-catalog → agentic each validate against their domain and codomain,
//! each commutes over the physics base (`p_membrane ∘ ψ = p_agentic ∘ χ =
//! p_catalog`), and the triangle commutes — `χ ∘ f = ψ` on every object and
//! morphism sort. A deliberately broken ψ fails the commutation check, so the
//! law discriminates rather than holding vacuously.
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

/// f: `agentic → membrane`.
fn f() -> GeometricMorphism {
    morphism("PANEL_KIT_MORPHISM")
}

#[test]
fn psi_and_chi_validate_on_their_domain_and_codomain() {
    let catalog = catalog();
    let agentic = topos("PANEL_KIT_TOPOS_A");
    let membrane = topos("PANEL_KIT_TOPOS_B");

    psi()
        .validate(&catalog, &membrane)
        .expect("psi is total on the catalog sorts and lands in the membrane sorts");
    chi()
        .validate(&catalog, &agentic)
        .expect("chi is total on the catalog sorts and lands in the agentic sorts");
}

#[test]
fn both_morphisms_commute_over_the_physics_base() {
    let p_catalog = morphism("PANEL_KIT_OKF_CATALOG_PHYSICS");
    let p_membrane = morphism("PANEL_KIT_MORPHISM_B_PHYSICS");
    let p_agentic = morphism("PANEL_KIT_MORPHISM_A_PHYSICS");

    psi().validate_over(&p_catalog, &p_membrane)
        .expect("p_membrane . psi = p_catalog on every sort");
    chi().validate_over(&p_catalog, &p_agentic)
        .expect("p_agentic . chi = p_catalog on every sort");

    // Pulled back along each morphism the base sorts are the catalog's own, so
    // all three regimes induce one physics section over the one base.
    assert_eq!(
        psi().inverse_image(&p_membrane.object_sorts),
        p_catalog.object_sorts
    );
    assert_eq!(
        chi().inverse_image(&p_agentic.object_sorts),
        p_catalog.object_sorts
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
fn the_two_regimes_declare_the_graph_component_and_external_panel() {
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