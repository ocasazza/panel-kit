# Single source of truth for the physics base stalks and the three geometric
# morphisms, so topos-physics.nix, the morphism-*.nix docs, and the regime
# "Morphism" text sheaves never drift. p_membrane . f = p_agentic on sorts.

rec {
  # Base engine stalks (the ONLY authored physics parameters).
  # repulsion high > low > minimal; weight stiff > lateral > weak.
  physics = {
    objects = {
      hydrophilic = { label = "Hydrophilic"; repulsion = 9.0; mass = 1.0; };
      hydrophobic = { label = "Hydrophobic"; repulsion = 3.0; mass = 1.5; };
      solvent = { label = "Solvent"; repulsion = 0.5; mass = 0.25; };
    };
    morphisms = {
      stiff = { label = "Stiff bond"; weight = 5.0; rest_length = 1.0; };
      lateral = { label = "Lateral bond"; weight = 2.5; rest_length = 1.4; };
      weak = { label = "Weak bond"; weight = 1.0; rest_length = 2.0; };
    };
  };

  # f: topos-agentic -> topos-membrane (non-injective: planner,reviewer |-> head).
  agenticToMembrane = {
    object_sorts = { planner = "head"; reviewer = "head"; coder = "tail"; memory = "solvent"; };
    morphism_sorts = { continues = "tail_bond"; shares = "lateral"; consolidates = "solvation"; };
  };

  # p_agentic: topos-agentic -> topos-physics (into the base).
  agenticToPhysics = {
    object_sorts = { planner = "hydrophilic"; reviewer = "hydrophilic"; coder = "hydrophobic"; memory = "solvent"; };
    morphism_sorts = { continues = "stiff"; shares = "lateral"; consolidates = "weak"; };
  };

  # p_membrane: topos-membrane -> topos-physics (into the base).
  membraneToPhysics = {
    object_sorts = { head = "hydrophilic"; tail = "hydrophobic"; solvent = "solvent"; };
    morphism_sorts = { tail_bond = "stiff"; lateral = "lateral"; solvation = "weak"; };
  };

  # Shortest decimal (trim trailing zeros) for prose; Nix toString pads to 6.
  fmtNum = x:
    let
      s = builtins.toString x;
      allZero = builtins.match "([0-9-]+)\\.0+" s;
      trimmed = builtins.match "(.*[.][0-9]*[1-9])0+" s;
    in
    if allZero != null then builtins.head allZero
    else if trimmed != null then builtins.head trimmed
    else s;

  # Morphism "f" translation table (lean-topos §16 style): agentic sort -> f* ->
  # membrane sort with the pulled-back engine tuning (== both regimes' tuning,
  # since p_membrane . f = p_agentic). Shared by both regime topoi.
  morphismHeading = "Morphism f: topos-agentic -> topos-membrane";
  morphismPreface =
    let
      objRow = aSort:
        let
          mSort = agenticToMembrane.object_sorts.${aSort};
          eng = agenticToPhysics.object_sorts.${aSort};
          p = physics.objects.${eng};
        in
        "  ${aSort} -> ${mSort}: repulsion ${fmtNum p.repulsion}, mass ${fmtNum p.mass} [f*: ${eng}]";
      morRow = k:
        let
          mSort = agenticToMembrane.morphism_sorts.${k};
          eng = agenticToPhysics.morphism_sorts.${k};
          w = physics.morphisms.${eng};
        in
        "  ${k} -> ${mSort}: weight ${fmtNum w.weight}, rest ${fmtNum w.rest_length} [f*: ${eng}]";
    in
    [ "p_membrane . f = p_agentic; tuning below is f* of the base stalks." "Objects (agentic -> membrane : pulled-back repulsion/mass [engine]):" ]
    ++ map objRow [ "planner" "reviewer" "coder" "memory" ]
    ++ [ "Morphisms (agentic -> membrane : pulled-back weight/rest [engine]):" ]
    ++ map morRow [ "continues" "shares" "consolidates" ];
}
