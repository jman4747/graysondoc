// The project function defines how your document looks.
#let project(
  title: "",
  authors: (),
  departments: (),
  doc_type: "",
  document_number: none,
  date: none,
  status: "DRAFT",
  version: "N/A",
  hash: "",
  src_length: 0,
  graysondoc_version: "0.1.0",
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
      #doc_type\-#document_number - #status - V#version
    ]
  )
  set text(font: "Atkinson Hyperlegible Next", lang: "en", size: 14pt)
  show link: underline
  let dept_line = for dept in departments {
    [
      *#dept*
    ]
  }
  // Title row.
  align(center)[
    #block(text(weight: 500, 1.5em, [#doc_type\-#document_number #title]))
    // #block(text(weight: 700, 1.75em, title))
    #v(2em, weak: true)
    #text([#dept_line], spacing: 200%) \
    #v(2em, weak: true)
    V#version - #date \
    #v(1em)
    // #image("../8100-logo.svg")
    // #text("So let it be written. So let it be Done.", style: "italic")
  ]
  pagebreak(weak: true)
  // table of contents
  outline()
  pagebreak(weak: true)
  heading("Document Control")
  align(left)[
    *Document Type:* #doc_type \
    *Document \#:* #document_number \
    *Status:* #status \
    *Version:* #version \
    *Source Repository:* https://github.com/ \
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
