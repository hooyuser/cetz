#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *

// Three-way composition: ((A ∪ B) ∩ C). Exercises nesting a path-bool
// drawable as the input of another path-bool.

#test-case({
  import draw: *

  let A-or-B = path-bool(
    { rect((0, 0), (1, 1)) },
    { rect((0.5, 0.5), (1.5, 1.5)) },
    op: "union",
    stroke: none,
  )

  path-bool(
    { A-or-B },
    { circle((1, 1), radius: 0.7) },
    op: "intersection",
    fill: rgb("#F4D6CC"),
    stroke: black,
  )
})
