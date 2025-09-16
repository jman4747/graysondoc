const BUILD_DIR_NAME = "build";
const PRELUDE_TEMPLATE = "((grayson.prelude))"

const YAML_TEMPLATE = "---
title: Grayson Pandoc
author:
((departments))
date: ((date))
fontsize: 12pt
toc: true
mainfont: AtkensonHyperlegibleNext
---
";

def main [input: path, ...authors: string] {
  let input_is_md = $input | path parse | get extension | $in == "md"

  if not $input_is_md {
    print $"Error: input \"($input)\" not markdown"
    return
  }

  let build_dir = ($input | path dirname | path join $BUILD_DIR_NAME)

  if not ($BUILD_DIR_NAME | path exists) {
    mkdir $build_dir
  }

  let parsed_input = ($input | path expand | path parse);

  let tmp_md = $build_dir | path join $"~($parsed_input.stem).md"

  if ($tmp_md | path exists) {
    print $"Warn: removing old temp markdown file at: ($tmp_md)"
    rm -fp $tmp_md 
  } 

  let raw_input = open --raw $input

  if not ($raw_input | str contains $DATE_TEMPLATE) {
    print $"Error: no date template found"
    return
  }

  if not ($raw_input | str contains $HASH_TEMPLATE ) {
    print $"Error: no source file hash template found"
    return
  }

  if not ($raw_input | str contains $AUTHORS_TEMPLATE ) {
    print $"Error: no authors template found"
    return
  }

  let hashed_input = $raw_input | hash sha256

  let authors_list = $authors | each {|name| $"- ($name)\n"} | str join

  open --raw $input |
  str replace $HASH_TEMPLATE $"## Source File Hash \(sha256\){-}\n($hashed_input)\n" |
  str replace $DATE_TEMPLATE $"(date now | format date "%Y-%m-%d")" |
  str replace $AUTHORS_TEMPLATE ([$"## Authors {-}", $authors_list, "\n"] | str join "\n") |
  save $tmp_md

  let out_file = [$parsed_input.parent, $BUILD_DIR_NAME, $"($parsed_input.stem).pdf"] | path join

  print $"Info: output file: ($out_file)"

  pandoc $tmp_md -f markdown --number-sections -o $out_file

  rm $tmp_md
}
