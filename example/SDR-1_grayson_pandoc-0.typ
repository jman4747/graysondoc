// The project function defines how your document looks.
#let project(
  title: "",
  authors: (:),
  departments: (:),
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
      #doc_type\-#document_number - #status - V: #version
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
    #text([#dept_line], spacing: 250%) \
    #v(2em, weak: true)
    #date \
    #v(1em)
    // #image("../8100-logo.svg")
    // #text("So let it be written. So let it be Done.", style: "italic")
  ]
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

#show: project.with(
	title: "Grayson Pandoc",
	authors: (
		"Josh T.",
		"Abcd E."
	),
	departments: (
		"IT",
		"Operations",
		"Outreach"
	),
	doc_type: "SDR",
	document_number: 0,
	date: "2025/11/20 UTC -0500",
	status: "DRAFT",
	version: 0,
	hash: "FFFF",
	src_length: 999,
	graysondoc_version: "0",
)

#heading(level: 1, numbering: none)[Objectives]
<objectives>
Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed in nibh a
erat iaculis ullamcorper.

= Section A
<section-a>
aliquam ultricies maximus metus, sit amet condimentum risus posuere sit
amet. pellentesque elit nisi, cursus eget suscipit eu, egestas eget
nibh. Pellentesque tincidunt nibh in libero blandit, eu feugiat est
dictum.

== Section AA
<section-aa>
curabitur tristique lobortis nisl, ut facilisis lacus tincidunt nec.
maecenas vitae elit ante. Sed erat neque, euismod sit amet volutpat nec,
aliquam at arcu.

= Section B
<section-b>
Sed euismod imperdiet felis vel congue. Maecenas porttitor nisi non
ultricies pulvinar. Sed massa mi, lacinia facilisis varius vitae,
vulputate ut mauris.
