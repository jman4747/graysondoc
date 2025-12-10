// The project function defines how your document looks.
#let project(
  title: "",
  authors: (),
  date: none,
  version: "N/A",
  hash: "",
  src_length: 0,
  graysondoc_version: "0.3.0",
  body
) = {
  // Set the document's basic properties.
  set document(author: authors, title: title)
  set page(
    paper: "us-letter",
    numbering: "1",
    number-align: center,
    header: [
      #set text(size: 8pt)
      #set align(center)
      V#version
    ]
  )
  set text(font: "Atkinson Hyperlegible Next", lang: "en", size: 14pt)
  show link: underline
  // Title row.
  align(center)[
    #block(text(weight: 500, 1.5em, [#title]))
    // #block(text(weight: 700, 1.75em, title))
    #v(2em, weak: true)
    V#version - #date \
    #v(1em)
    // #image("../8100-logo.svg")
    // #text("So let it be written. So let it be Done.", style: "italic")
  ]
  // table of contents
  outline()
  pagebreak(weak: true)
  heading("Document Control")
  align(left)[
    *Version:* #version \
    *Souce File Hash (XXH3 64):* #hash \
    *Souce File Length:* #src_length \
    *Graysondoc Version:* #graysondoc_version \
  ]
  heading("Authors")
  for author in authors {
    [
      *#author* \
    ]
  }
  set par(justify: true)
  set heading(numbering: none)
  // Main body.
  pagebreak(weak: true)
  set heading(numbering: "1.1")
  body
}
