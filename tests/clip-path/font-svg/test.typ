#set page(width: auto, height: auto)
#import "/src/lib.typ": *
#import "/tests/helper.typ": *


// // Convert an SVG path `d="..."` string to CeTZ svg-path commands.
// // Normalizes everything to absolute commands: M, L, C, Q, Z.
// // Expands SVG smooth commands S/s -> C and T/t -> Q.
// #let svg-d-to-cetz(d) = {
//   let token-re = regex("[AaCcHhLlMmQqSsTtVvZz]|[-+]?(?:\\d*\\.\\d+|\\d+)(?:[eE][-+]?\\d+)?")
//   let command-re = regex("^[AaCcHhLlMmQqSsTtVvZz]$")

//   let tokens = d.matches(token-re).map(m => m.text)

//   let i = 0
//   let cmd = none
//   let out = ()

//   let cur = (0.0, 0.0)
//   let substart = (0.0, 0.0)

//   let last-cubic-ctrl = none
//   let last-quad-ctrl = none

//   let is-command(tok) = tok.matches(command-re).len() > 0

//   let num() = {
//     if i >= tokens.len() {
//       panic("SVG path ended while a number was expected")
//     }

//     let tok = tokens.at(i)
//     if is-command(tok) {
//       panic("Expected number, got command `" + tok + "`")
//     }

//     i += 1
//     float(tok)
//   }

//   let pair() = (num(), num())

//   let add(a, b) = (
//     a.at(0) + b.at(0),
//     a.at(1) + b.at(1),
//   )

//   let abs-point(p, rel) = {
//     if rel { add(cur, p) } else { p }
//   }

//   let reflect(p, about) = {
//     if p == none {
//       about
//     } else {
//       (
//         2.0 * about.at(0) - p.at(0),
//         2.0 * about.at(1) - p.at(1),
//       )
//     }
//   }

//   let reset-smooth() = {
//     last-cubic-ctrl = none
//     last-quad-ctrl = none
//   }

//   while i < tokens.len() {
//     if is-command(tokens.at(i)) {
//       cmd = tokens.at(i)
//       i += 1
//     }

//     if cmd == none {
//       panic("SVG path must start with a command")
//     }

//     if cmd in ("M", "m") {
//       let rel = cmd == "m"
//       let p = abs-point(pair(), rel)

//       out.push(("M", p))
//       cur = p
//       substart = p
//       reset-smooth()

//       // Further pairs after M/m are implicit L/l.
//       cmd = if rel { "l" } else { "L" }
//     } else if cmd in ("L", "l") {
//       let rel = cmd == "l"
//       let p = abs-point(pair(), rel)

//       out.push(("L", p))
//       cur = p
//       reset-smooth()
//     } else if cmd in ("H", "h") {
//       let rel = cmd == "h"
//       let x = num()
//       let p = if rel {
//         (cur.at(0) + x, cur.at(1))
//       } else {
//         (x, cur.at(1))
//       }

//       out.push(("L", p))
//       cur = p
//       reset-smooth()
//     } else if cmd in ("V", "v") {
//       let rel = cmd == "v"
//       let y = num()
//       let p = if rel {
//         (cur.at(0), cur.at(1) + y)
//       } else {
//         (cur.at(0), y)
//       }

//       out.push(("L", p))
//       cur = p
//       reset-smooth()
//     } else if cmd in ("C", "c") {
//       let rel = cmd == "c"
//       let c1 = abs-point(pair(), rel)
//       let c2 = abs-point(pair(), rel)
//       let p = abs-point(pair(), rel)

//       out.push(("C", c1, c2, p))
//       cur = p
//       last-cubic-ctrl = c2
//       last-quad-ctrl = none
//     } else if cmd in ("S", "s") {
//       let rel = cmd == "s"

//       // SVG S uses reflection of previous cubic control point.
//       let c1 = reflect(last-cubic-ctrl, cur)
//       let c2 = abs-point(pair(), rel)
//       let p = abs-point(pair(), rel)

//       // CeTZ has no S in your listed API, so emit equivalent C.
//       out.push(("C", c1, c2, p))
//       cur = p
//       last-cubic-ctrl = c2
//       last-quad-ctrl = none
//     } else if cmd in ("Q", "q") {
//       let rel = cmd == "q"
//       let c = abs-point(pair(), rel)
//       let p = abs-point(pair(), rel)

//       out.push(("Q", c, p))
//       cur = p
//       last-quad-ctrl = c
//       last-cubic-ctrl = none
//     } else if cmd in ("T", "t") {
//       let rel = cmd == "t"

//       // SVG T uses reflection of previous quadratic control point.
//       let c = reflect(last-quad-ctrl, cur)
//       let p = abs-point(pair(), rel)

//       out.push(("Q", c, p))
//       cur = p
//       last-quad-ctrl = c
//       last-cubic-ctrl = none
//     } else if cmd in ("Z", "z") {
//       // Important: in Typst, one-element arrays need the trailing comma.
//       out.push(("Z",))
//       cur = substart
//       reset-smooth()
//       cmd = none
//     } else if cmd in ("A", "a") {
//       panic("SVG arc command A/a is not supported by this converter")
//     } else {
//       panic("Unsupported SVG command `" + cmd + "`")
//     }
//   }

//   out
// }

// Optional helper for a full <path ... d="..." .../> string.
// This extracts only the double-quoted d= attribute.
// #let svg-path-tag-to-cetz(tag) = {
//   let matches = tag.matches(regex("d\\s*=\\s*\"([^\"]*)\""))
//   if matches.len() == 0 {
//     panic("No double-quoted d=\"...\" attribute found")
//   }

//   svg-d-to-cetz(matches.at(0).captures.at(0))
// }

#test-case({
  import draw: *

  let svg1 = scope({
    rotate(x: 180deg)
    svg-path(
      name: "my-svg-path",
      stroke: 4pt + orange.desaturate(20%),
      fill: blue,
      fill-rule: "even-odd",
      ("M", (5.08, 7)),
      ("L", (3.96, 7)),
      ("L", (3.96, 3.44)),
      ("Q", (3.96, 2.71), (3.705, 2.245)),
      ("Q", (3.45, 1.78), (3.035, 1.515)),
      ("Q", (2.62, 1.25), (2.125, 1.14)),
      ("Q", (1.63, 1.03), (1.15, 1.02)),
      ("L", (1.15, 7)),
      ("L", (0, 7)),
      ("L", (0, 0)),
      ("L", (0.97, 0)),
      ("Q", (1.73, 0), (2.535, 0.185)),
      ("Q", (3.34, 0.37), (3.96, 0.86)),
      ("L", (3.96, 0)),
      ("L", (4.93, 0)),
      ("Q", (5.48, 0), (6.055, 0.085)),
      ("Q", (6.63, 0.17), (7.16, 0.385)),
      ("Q", (7.69, 0.6), (8.115, 0.98)),
      ("Q", (8.54, 1.36), (8.79, 1.94)),
      ("Q", (9.04, 2.52), (9.04, 3.35)),
      ("L", (9.04, 7)),
      ("L", (7.88, 7)),
      ("L", (7.88, 3.36)),
      ("Q", (7.88, 2.62), (7.625, 2.16)),
      ("Q", (7.37, 1.7), (6.955, 1.455)),
      ("Q", (6.54, 1.21), (6.05, 1.12)),
      ("Q", (5.56, 1.03), (5.08, 1.02)),
      ("L", (5.08, 7)),
      ("Z",),
      ("M", (14.75, 7)),
      ("L", (14.15, 7)),
      ("Q", (13.6, 7), (13.025, 6.915)),
      ("Q", (12.45, 6.83), (11.92, 6.615)),
      ("Q", (11.39, 6.4), (10.965, 6.02)),
      ("Q", (10.54, 5.64), (10.29, 5.06)),
      ("Q", (10.04, 4.48), (10.04, 3.65)),
      ("L", (10.04, 0)),
      ("L", (14.76, 0)),
      ("L", (14.76, 1.07)),
      ("L", (11.2, 1.07)),
      ("L", (11.2, 2.43)),
      ("L", (14.05, 2.43)),
      ("L", (14.05, 3.45)),
      ("L", (11.2, 3.45)),
      ("Q", (11.2, 4.26), (11.44, 4.765)),
      ("Q", (11.68, 5.27), (12.095, 5.53)),
      ("Q", (12.51, 5.79), (13.04, 5.885)),
      ("Q", (13.57, 5.98), (14.15, 5.98)),
      ("L", (14.75, 5.98)),
      ("L", (14.75, 7)),
      ("Z",),
      ("M", (22.43, 7)),
      ("L", (21.27, 7)),
      ("L", (21.27, 0)),
      ("L", (22.43, 0)),
      ("Q", (22.98, 0), (23.545, 0.09)),
      ("Q", (24.11, 0.18), (24.635, 0.395)),
      ("Q", (25.16, 0.61), (25.575, 0.995)),
      ("Q", (25.99, 1.38), (26.23, 1.955)),
      ("Q", (26.47, 2.53), (26.47, 3.35)),
      ("L", (26.47, 7)),
      ("L", (25.31, 7)),
      ("L", (25.31, 4.67)),
      ("L", (22.43, 4.67)),
      ("L", (22.43, 7)),
      ("Z",),
      ("M", (20.37, 5.92)),
      ("L", (20.37, 7.01)),
      ("Q", (19.74, 6.97), (19.115, 6.79)),
      ("Q", (18.49, 6.61), (17.975, 6.225)),
      ("Q", (17.46, 5.84), (17.145, 5.215)),
      ("Q", (16.83, 4.59), (16.83, 3.67)),
      ("L", (16.83, 1.11)),
      ("L", (15.87, 1.11)),
      ("L", (15.87, 0)),
      ("L", (20.07, 0)),
      ("L", (20.07, 1.11)),
      ("L", (17.99, 1.11)),
      ("L", (17.99, 3.66)),
      ("Q", (17.99, 4.3), (18.195, 4.725)),
      ("Q", (18.4, 5.15), (18.745, 5.4)),
      ("Q", (19.09, 5.65), (19.515, 5.77)),
      ("Q", (19.94, 5.89), (20.37, 5.92)),
      ("Z",),
      ("M", (22.43, 1.02)),
      ("Q", (22.91, 1.02), (23.415, 1.11)),
      ("Q", (23.92, 1.2), (24.35, 1.445)),
      ("Q", (24.78, 1.69), (25.045, 2.15)),
      ("Q", (25.31, 2.61), (25.31, 3.36)),
      ("L", (25.31, 3.57)),
      ("L", (22.43, 3.57)),
      ("L", (22.43, 1.02)),
      ("Z",),
    )
  })

  let svg2 = scope({
    rotate(x: 180deg)
    svg-path(
      name: "my-svg-path",
      stroke: 3pt + blue,
      fill: red,
      fill-rule: "even-odd",
      ("M", (5.08, 7)),
      ("L", (3.96, 7)),
      ("L", (3.96, 3.44)),
      ("Q", (3.96, 2.71), (3.705, 2.245)),
      ("Q", (3.45, 1.78), (3.035, 1.515)),
      ("Q", (2.62, 1.25), (2.125, 1.14)),
      ("Q", (1.63, 1.03), (1.15, 1.02)),
      ("L", (1.15, 7)),
      ("L", (0, 7)),
      ("L", (0, 0)),
      ("L", (0.97, 0)),
      ("Q", (1.73, 0), (2.535, 0.185)),
      ("Q", (3.34, 0.37), (3.96, 0.86)),
      ("L", (3.96, 0)),
      ("L", (4.93, 0)),
      ("Q", (5.48, 0), (6.055, 0.085)),
      ("Q", (6.63, 0.17), (7.16, 0.385)),
      ("Q", (7.69, 0.6), (8.115, 0.98)),
      ("Q", (8.54, 1.36), (8.79, 1.94)),
      ("Q", (9.04, 2.52), (9.04, 3.35)),
      ("L", (9.04, 7)),
      ("L", (7.88, 7)),
      ("L", (7.88, 3.36)),
      ("Q", (7.88, 2.62), (7.625, 2.16)),
      ("Q", (7.37, 1.7), (6.955, 1.455)),
      ("Q", (6.54, 1.21), (6.05, 1.12)),
      ("Q", (5.56, 1.03), (5.08, 1.02)),
      ("L", (5.08, 7)),
      ("Z",),
      ("M", (14.75, 7)),
      ("L", (14.15, 7)),
      ("Q", (13.6, 7), (13.025, 6.915)),
      ("Q", (12.45, 6.83), (11.92, 6.615)),
      ("Q", (11.39, 6.4), (10.965, 6.02)),
      ("Q", (10.54, 5.64), (10.29, 5.06)),
      ("Q", (10.04, 4.48), (10.04, 3.65)),
      ("L", (10.04, 0)),
      ("L", (14.76, 0)),
      ("L", (14.76, 1.07)),
      ("L", (11.2, 1.07)),
      ("L", (11.2, 2.43)),
      ("L", (14.05, 2.43)),
      ("L", (14.05, 3.45)),
      ("L", (11.2, 3.45)),
      ("Q", (11.2, 4.26), (11.44, 4.765)),
      ("Q", (11.68, 5.27), (12.095, 5.53)),
      ("Q", (12.51, 5.79), (13.04, 5.885)),
      ("Q", (13.57, 5.98), (14.15, 5.98)),
      ("L", (14.75, 5.98)),
      ("L", (14.75, 7)),
      ("Z",),
      ("M", (22.43, 7)),
      ("L", (21.27, 7)),
      ("L", (21.27, 0)),
      ("L", (22.43, 0)),
      ("Q", (22.98, 0), (23.545, 0.09)),
      ("Q", (24.11, 0.18), (24.635, 0.395)),
      ("Q", (25.16, 0.61), (25.575, 0.995)),
      ("Q", (25.99, 1.38), (26.23, 1.955)),
      ("Q", (26.47, 2.53), (26.47, 3.35)),
      ("L", (26.47, 7)),
      ("L", (25.31, 7)),
      ("L", (25.31, 4.67)),
      ("L", (22.43, 4.67)),
      ("L", (22.43, 7)),
      ("Z",),
      ("M", (20.37, 5.92)),
      ("L", (20.37, 7.01)),
      ("Q", (19.74, 6.97), (19.115, 6.79)),
      ("Q", (18.49, 6.61), (17.975, 6.225)),
      ("Q", (17.46, 5.84), (17.145, 5.215)),
      ("Q", (16.83, 4.59), (16.83, 3.67)),
      ("L", (16.83, 1.11)),
      ("L", (15.87, 1.11)),
      ("L", (15.87, 0)),
      ("L", (20.07, 0)),
      ("L", (20.07, 1.11)),
      ("L", (17.99, 1.11)),
      ("L", (17.99, 3.66)),
      ("Q", (17.99, 4.3), (18.195, 4.725)),
      ("Q", (18.4, 5.15), (18.745, 5.4)),
      ("Q", (19.09, 5.65), (19.515, 5.77)),
      ("Q", (19.94, 5.89), (20.37, 5.92)),
      ("Z",),
      ("M", (22.43, 1.02)),
      ("Q", (22.91, 1.02), (23.415, 1.11)),
      ("Q", (23.92, 1.2), (24.35, 1.445)),
      ("Q", (24.78, 1.69), (25.045, 2.15)),
      ("Q", (25.31, 2.61), (25.31, 3.36)),
      ("L", (25.31, 3.57)),
      ("L", (22.43, 3.57)),
      ("L", (22.43, 1.02)),
      ("Z",),
    )
  })


  let circ = circle((12.7, -2.3), radius: 230pt, stroke: 4pt + red.desaturate(20%))
  circ
  clip-path(
    circ,
    svg1,
    mode: "include",
  )
  clip-path(
    circ,
    svg2,
    mode: "exclude",
  )
})