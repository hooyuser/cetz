#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *

#test-case({
  import draw: *

  let clip = { circle((0, 0), radius: 1.1) }

  circle((0, 0), radius: 1.1, stroke: 0.35pt + gray, fill: none)
  clip-path(
    clip,
    {
      line((-1.6, 0), (1.6, 0), stroke: 2pt + blue)
      rect((-0.95, -0.55), (0.35, 0.55), fill: rgb("#b7e4c7"), stroke: none)
      rect((-0.25, -1.25), (1.35, 0.25), fill: none, stroke: 1.5pt + rgb("#7b2cbf"))
    },
    mode: "include",
    eps: 1e-6,
  )

  set-origin((0, -3))
  circle((0, 0), radius: 1.1, stroke: 0.35pt + gray, fill: none)
  clip-path(
    clip,
    { line((-1.6, 0), (1.6, 0), stroke: 2pt + blue) },
    mode: "include",
    eps: 1e-6,
  )
  clip-path(
    clip,
    { rect((-0.95, -0.55), (0.35, 0.55), fill: rgb("#b7e4c7"), stroke: none) },
    mode: "include",
    eps: 1e-6,
  )
  clip-path(
    clip,
    { rect((-0.25, -1.25), (1.35, 0.25), fill: none, stroke: 1.5pt + rgb("#7b2cbf")) },
    mode: "include",
    eps: 1e-6,
  )
})
