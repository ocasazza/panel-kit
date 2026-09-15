{ lib ? (import <nixpkgs> { }).lib }:

let
  workspaceSpecSchema = builtins.fromJSON (builtins.readFile ../schema/workspace-spec.schema.json);
  schemaValidator = import ../lib/validateSchema.nix { inherit lib; };
  good = (import ./workspace-spec.nix { inherit lib; }).value;
  badgePanel = builtins.elemAt good.panels 3;
  badge = builtins.head badgePanel.content.source.value;
  badgeSchema = workspaceSpecSchema // { "$ref" = "#/$defs/BadgeSpec"; };

  assertHasError = expected: errors:
    if builtins.elem expected errors then true
    else throw "expected schema validation error ${expected}, got ${builtins.toJSON errors}";

  assertNoErrors = name: errors:
    if errors == [] then true
    else throw "expected ${name} to pass schema validation, got ${builtins.toJSON errors}";
in
{
  explicit_null = assertNoErrors "explicit null nullable badge fields" (
    schemaValidator.validate badgeSchema badge
  );

  missing_override_color = assertHasError
    "/override_color: missing required field"
    (schemaValidator.validate badgeSchema (builtins.removeAttrs badge [ "override_color" ]));

  missing_accent_color = assertHasError
    "/accent_color: missing required field"
    (schemaValidator.validate badgeSchema (builtins.removeAttrs badge [ "accent_color" ]));
}
