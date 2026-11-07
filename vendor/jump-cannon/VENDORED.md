# Vendored from jump-cannon

Source: https://github.com/ocasazza/jump-cannon (MIT, as declared in each
crate's manifest). Used by `examples/autoloop.rs` for its graph panel.

| Here | Upstream path | Upstream revision |
|---|---|---|
| `graph-view/src` | `app/graph-view/src` | `feat/per-node-physics` @ `3fdc0af` |
| `graph-layouts/src`, `graph-layouts/README.md` | `crates/graph-layouts/` | `feat/per-node-physics` @ `3fdc0af` |

Sources are copied unmodified. Manifest changes, and nothing else:

- `graph-view/Cargo.toml`: jump-cannon `app/` workspace inheritance replaced by
  the same concrete values; `graph-layouts` points at the vendored copy.
- `graph-layouts/Cargo.toml`: dev-dependency and example sections removed; their
  tests, examples and benches are not vendored.

Not vendored: `crates/graph-layouts/{examples,tests,docs,mock-generator}`.

Both crates sit outside this workspace (`exclude = ["vendor"]`), so panel-kit's
lints and tests do not apply to them. Changes go upstream first, then re-copy:

```sh
JC=../jump-cannon
rm -rf vendor/jump-cannon/graph-view/src vendor/jump-cannon/graph-layouts/src
cp -R "$JC/app/graph-view/src" vendor/jump-cannon/graph-view/
cp -R "$JC/crates/graph-layouts/src" "$JC/crates/graph-layouts/README.md" vendor/jump-cannon/graph-layouts/
```

and update the revisions above.
