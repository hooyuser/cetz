// Direct smoke test of `cetz-core.path_bool_func`, bypassing the CeTZ draw
// pipeline. Verifies that:
//   1. CBOR encoding from Typst is accepted by the WASM module
//   2. The four boolean ops on a pair of overlapping unit squares each
//      produce a path with the expected number of subpaths
//   3. The wire format decoded back into Typst has the expected shape

#set page(width: auto, height: auto)
#import "/src/wasm.typ": call_wasm
#let cetz-core = plugin("/cetz-core/cetz_core.wasm")

#let unit-rect(x, y) = (
  origin: (x, y),
  closed: true,
  segments: (
    (kind: "l", to: (x + 1.0, y)),
    (kind: "l", to: (x + 1.0, y + 1.0)),
    (kind: "l", to: (x, y + 1.0)),
  ),
)

#let run(op) = call_wasm(cetz-core.path_bool_func, (
  a: (subpaths: (unit-rect(0.0, 0.0),)),
  b: (subpaths: (unit-rect(0.5, 0.5),)),
  op: op,
  fill_rule_a: "non-zero",
  fill_rule_b: "non-zero",
  eps: none,
))

#let union-result = run("union")
#assert("path" in union-result, message: "result missing `path` field")
#assert("subpaths" in union-result.path, message: "path missing `subpaths`")
#assert.eq(
  union-result.path.subpaths.len(),
  1,
  message: "union of overlapping squares should produce 1 subpath",
)

#let inter-result = run("intersection")
#assert.eq(inter-result.path.subpaths.len(), 1)

#let diff-result = run("difference")
#assert.eq(diff-result.path.subpaths.len(), 1)

#let xor-result = run("xor")
#assert(xor-result.path.subpaths.len() >= 1)

// Validate one segment shape end-to-end. Lines round-trip as
// `(kind: "l", to: (x, y))` per our serde tagged-enum encoding.
#let first-seg = union-result.path.subpaths.at(0).segments.at(0)
#assert.eq(first-seg.kind, "l")
#assert(type(first-seg.to) == array and first-seg.to.len() == 2)

// Render a deterministic indicator so tytanic has page output to compare.
#rect(width: 2cm, height: 1cm, stroke: 0.5pt + black, inset: 4pt)[
  #text(8pt)[path-bool wasm smoke OK]
]
