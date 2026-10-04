# Single source of truth for the physics base stalks and the geometric
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

  # p_trace: topos-autoloop-trace -> topos-physics. Sessions assemble; their
  # repo, goal and gate anchors face outward; events are solvent.
  autoloopTraceToPhysics = {
    object_sorts = {
      session = "hydrophobic"; goal = "hydrophilic"; repo = "hydrophilic"; gate = "hydrophilic";
      event = "solvent"; elsewhere = "solvent";
    };
    morphism_sorts = {
      next = "stiff"; emitted = "stiff"; spawned = "stiff"; in_repo = "lateral"; pursues = "lateral";
      runs_gate = "weak"; elsewhere = "weak";
    };
  };

  # p_control: topos-autoloop-control -> topos-physics. Control state has no
  # assembly of its own: every sort is solvent, every bond weak.
  autoloopControlToPhysics =
    let
      all = value: names: builtins.listToAttrs (map (name: { inherit name value; }) names);
    in
    {
      object_sorts = all "solvent" [ "claim" "proposal" "queued_goal" "repo" "repo_policy" "term" "param" "prompt" "elsewhere" ];
      morphism_sorts = all "weak" [ "targets" "governs" "revises" "elsewhere" ];
    };
}
