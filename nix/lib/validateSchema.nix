{ lib }:

let
  pointerJoin = pointer: segment:
    let escaped = builtins.replaceStrings [ "~" "/" ] [ "~0" "~1" ] (toString segment);
    in if pointer == "" then "/${escaped}" else "${pointer}/${escaped}";

  schemaType = value:
    let ty = builtins.typeOf value;
    in if ty == "int" then "integer"
       else if ty == "float" then "number"
       else if ty == "set" then "object"
       else if ty == "list" then "array"
       else if ty == "bool" then "boolean"
       else ty;

  acceptsType = expected: value:
    let actual = schemaType value;
    in if builtins.isList expected then builtins.any (type: acceptsType type value) expected
       else if expected == "number" then actual == "number" || actual == "integer"
       else expected == actual;

  has = set: name: builtins.hasAttr name set;

  refName = ref:
    let prefix = "#/$defs/";
    in if lib.hasPrefix prefix ref then builtins.substring (builtins.stringLength prefix) (-1) ref
       else throw "validateSchema: unsupported schema ref ${ref}";

  validate = root: value:
    let
      resolve = schema:
        if has schema "$ref" then root."$defs".${refName schema."$ref"} else schema;

      validateAt = schema': pointer: current:
        let schema = resolve schema';
        in
          if has schema "oneOf" then validateOneOf schema.oneOf pointer current
          else
            lib.optionals (has schema "type" && !(acceptsType schema.type current)) [
              "${pointer}: expected ${builtins.toJSON schema.type}, got ${schemaType current}"
            ]
            ++ lib.optionals (has schema "const" && current != schema.const) [
              "${pointer}: expected constant ${builtins.toJSON schema.const}"
            ]
            ++ lib.optionals (has schema "enum" && !(builtins.elem current schema.enum)) [
              "${pointer}: expected one of ${builtins.toJSON schema.enum}"
            ]
            ++ validateObject schema pointer current
            ++ validateArray schema pointer current;

      validateObject = schema: pointer: current:
        if !(builtins.isAttrs current) || current == null then []
        else
          let
            properties = schema.properties or {};
            propertyNames = builtins.attrNames properties;
            required = schema.required or [];
            unknown = lib.subtractLists propertyNames (builtins.attrNames current);
            extraErrors = lib.optionals ((schema.additionalProperties or true) == false) (
              builtins.map (name: "${pointerJoin pointer name}: unknown field; allowed fields: ${lib.concatStringsSep ", " propertyNames}") unknown
            );
            missingErrors = builtins.map (name: "${pointerJoin pointer name}: missing required field") (
              builtins.filter (name: !(has current name)) required
            );
            fieldErrors = lib.concatMap (name: validateAt properties.${name} (pointerJoin pointer name) current.${name}) (
              builtins.filter (name: has properties name) (builtins.attrNames current)
            );
          in extraErrors ++ missingErrors ++ fieldErrors;

      validateArray = schema: pointer: current:
        if !(builtins.isList current) then []
        else
          let
            length = builtins.length current;
            minErrors = lib.optionals (has schema "minItems" && length < schema.minItems) [
              "${pointer}: expected at least ${toString schema.minItems} items"
            ];
            maxErrors = lib.optionals (has schema "maxItems" && length > schema.maxItems) [
              "${pointer}: expected at most ${toString schema.maxItems} items"
            ];
            itemErrors =
              if has schema "prefixItems" then
                lib.concatMap (index:
                  let itemSchema = builtins.elemAt schema.prefixItems index;
                  in if index < length then validateAt itemSchema (pointerJoin pointer index) (builtins.elemAt current index) else []
                ) (lib.range 0 ((builtins.length schema.prefixItems) - 1))
              else if has schema "items" then
                if length == 0 then [] else
                  lib.concatMap (index: validateAt schema.items (pointerJoin pointer index) (builtins.elemAt current index))
                    (lib.range 0 (length - 1))
              else [];
          in minErrors ++ maxErrors ++ itemErrors;

      validateOneOf = alternatives: pointer: current:
        let
          discriminated = selectDiscriminated alternatives current;
          candidates = if discriminated == [] then alternatives else discriminated;
          successes = builtins.filter (errors: errors == []) (
            builtins.map (alternative: validateAt alternative pointer current) candidates
          );
        in if successes != [] then [] else [ "${pointer}: value did not match any schema alternative" ];

      selectDiscriminated = alternatives: current:
        if !(builtins.isAttrs current) || current == null then []
        else
          let
            matchConst = alternative:
              let schema = resolve alternative;
              in if has schema "properties" && has schema.properties "kind" && has schema.properties.kind "const" && has current "kind" then
                   current.kind == schema.properties.kind.const
                 else if has schema "properties" && has schema.properties "source" && has schema.properties.source "const" && has current "source" then
                   current.source == schema.properties.source.const
                 else false;
          in builtins.filter matchConst alternatives;
    in validateAt root "" value;
in
{
  inherit validate;
}
