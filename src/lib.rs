use std::{
	borrow::Cow,
	ffi::OsStr,
	fs::OpenOptions,
	io::Read,
	path::{Path, PathBuf},
	process::Command,
};

use argh::FromArgs;
use chrono::{DateTime, Local};
use nom::{
	IResult, Parser,
	character::complete::{alpha1, char, usize},
	sequence::separated_pair,
};
use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("input: {0:?} isn't a file")]
	InputIsFile(Box<Path>),
	#[error("input: {0:?} dosen't exist")]
	InputExists(Box<Path>),
	#[error("input file name: {0:?} isn't markdown")]
	IsMd(Box<Path>),
	#[error("input file name: {name:?} format error:\n{ne:}")]
	ParseFileName {
		name: Box<str>,
		#[source]
		ne: nom::Err<nom::error::Error<String>>,
	},
	#[error("pandoc executable not available:\n{0:?}")]
	PandocVersionCmd(#[source] std::io::Error),
	#[error("pandoc executable not available:\n{0}")]
	PandocVersionOutput(Box<str>),
	#[error("pdflatex executable not available:\n{0:?}")]
	LatexVersionCmd(#[source] std::io::Error),
	#[error("pdflatex executable not available:\n{0:?}")]
	LatexVersionOutput(Box<str>),
	#[error("can't read input markdown file:\n{0:?}")]
	ReadInputMd(#[source] std::io::Error),
	#[error("first line must be exactly \"# Objectives\" but was: {0:?}")]
	BadFirstSection(Box<str>),
	#[error("no metadata toml file at: {0:?}")]
	CheckToml(Box<Path>),
	#[error("can't open metadata toml file at: {path:?}:\n{ioe}")]
	OpenMetadata {
		#[source]
		ioe: std::io::Error,
		path: Box<Path>,
	},
	#[error("can't read metadata toml file at: {path:?}:\n{ioe}")]
	ReadMetadata {
		#[source]
		ioe: std::io::Error,
		path: Box<Path>,
	},
	#[error("can't parse metadata toml file at: {path:?}:\n{tpem}")]
	ParseMetadata { tpem: Box<str>, path: Box<Path> },
	#[error("pandoc executable not available:\n{0:?}")]
	PandocInvoke(std::io::Error),
	#[error("pandoc executable not available:\n{0}")]
	PandocOutput(Box<str>),
}

pub const METADATA_FILE_NAME: &str = "metadata.toml";

#[cfg(not(target_os = "windows"))]
pub const PANDOC_CMD: &str = "pandoc";
#[cfg(target_os = "windows")]
pub const PANDOC_CMD: &str = "pandoc.exe";
#[cfg(not(target_os = "windows"))]
pub const LATEX_CMD: &str = "lualatex";
#[cfg(target_os = "windows")]
pub const LATEX_CMD: &str = "lualatex.exe";

#[derive(FromArgs, PartialEq, Debug)]
/// Graysondoc
pub struct GdocCli {
	/// markdown file to convert
	#[argh(positional)]
	pub input_md: PathBuf,
	/// revision number
	#[argh(option, short = 'r')]
	pub revision: usize,
	/// alternate toml metadata file
	#[argh(option, short = 'm')]
	pub metadata: Option<PathBuf>,
	/// print version information
	#[argh(switch)]
	pub version: bool,
	/// don't delete markdown itermediate representation
	#[argh(switch)]
	pub preserve_ir: bool,
	/// stop after building markdown itermediate representation, implies --preserve-ir
	#[argh(switch)]
	pub ir_only: bool,
}

#[derive(Debug)]
pub struct ParsedMdName<'name> {
	pub code: &'name str,
	pub number: usize,
	pub title: Cow<'name, str>,
}

pub fn nom_document_name(input: &str) -> IResult<&str, ParsedMdName<'_>> {
	let mut code_parser = separated_pair(alpha1, char('-'), usize);
	let (rst, (code, number)) = code_parser.parse(input)?;
	let (raw_title, _underscore) = char('_').parse(rst)?;

	let raw_trim_title = raw_title.trim().trim_matches(&['_', '＿']);

	let title: Cow<'_, str> = if raw_trim_title.contains('_') {
		let mut buf = String::with_capacity(raw_trim_title.len());
		let mut chars = raw_trim_title.chars();
		if let Some(ch) = chars.next() {
			// first to upper
			buf.push(ch.to_ascii_uppercase());
		}
		let mut after_space = false;
		while let Some(ch) = chars.next() {
			if ch == '_' || ch == '＿' {
				buf.push(' ');
				after_space = true;
			} else if after_space {
				buf.push(ch.to_ascii_uppercase());
				after_space = false;
			} else {
				buf.push(ch);
			}
		}
		Cow::Owned(buf)
	} else {
		Cow::Borrowed(raw_trim_title)
	};

	Ok((
		input,
		ParsedMdName {
			code,
			number,
			title,
		},
	))
}

pub fn check_for_exes() -> Result<(), Error> {
	let pandoc_version = Command::new(PANDOC_CMD)
		.arg("--version")
		.output()
		.map_err(|ioe| Error::PandocVersionCmd(ioe))
		.inspect_err(|e| eprintln!("{e}"))?;

	if pandoc_version.status.success() {
		const ERR_MSG: &str = "can't read stdout stream from pandoc";
		let version_msg: String = pandoc_version
			.stdout
			.try_into()
			.map_err(|_| Error::PandocVersionOutput(Box::from(ERR_MSG)))?;
		let first_line = version_msg
			.lines()
			.next()
			.ok_or_else(|| Error::PandocVersionOutput(Box::from(ERR_MSG)))?;
		println!("With: {first_line}");
	} else {
		let e_msg: String = pandoc_version
			.stderr
			.try_into()
			.unwrap_or_else(|_| "unknown error getting pandoc version".into());
		return Err(Error::PandocVersionOutput(e_msg.into_boxed_str()));
	}

	let latex_version = Command::new(LATEX_CMD)
		.arg("--version")
		.output()
		.map_err(|ioe| Error::LatexVersionCmd(ioe))?;

	if latex_version.status.success() {
		const ERR_MSG: &str = "can't read stdout stream from pdflatex";
		let version_msg: String = latex_version
			.stdout
			.try_into()
			.map_err(|_| Error::LatexVersionOutput(Box::from(ERR_MSG)))?;
		let first_line = version_msg
			.lines()
			.next()
			.ok_or_else(|| Error::LatexVersionOutput(Box::from(ERR_MSG)))?;
		println!("With: {first_line}");
	} else {
		let e_msg: String = latex_version
			.stderr
			.try_into()
			.unwrap_or_else(|_| "unknown error getting pdflatex version".into());
		return Err(Error::LatexVersionOutput(e_msg.into_boxed_str()));
	}
	Ok(())
}

pub fn call_pandoc(
	irmd_path: impl AsRef<OsStr>,
	ouput_path: impl AsRef<OsStr>,
) -> Result<(), Error> {
	// pandoc $irmd -f markdown --number-sections --pdf-engine=lualatex -o $output.pdf
	let pandoc_invoke = Command::new(PANDOC_CMD)
		.arg(irmd_path)
		.arg("-f")
		.arg("markdown")
		.arg("--number-sections")
		.arg("--pdf-engine=lualatex")
		.arg("-o")
		.arg(ouput_path)
		.output()
		.map_err(|ioe| Error::PandocInvoke(ioe))
		.inspect_err(|e| eprintln!("{e}"))?;

	if pandoc_invoke.status.success() {
		const ERR_MSG: &str = "can't read stdout stream from pandoc";
		let pandoc_out: String = pandoc_invoke
			.stdout
			.try_into()
			.map_err(|_| Error::PandocOutput(Box::from(ERR_MSG)))?;
		let pandoc_err: String = pandoc_invoke
			.stderr
			.try_into()
			.map_err(|_| Error::PandocOutput(Box::from(ERR_MSG)))?;
		if pandoc_err.len() > 0 || pandoc_out.len() > 0 {
			println!("Pandoc Output:");
		}
		if pandoc_err.len() > 0 {
			println!("{pandoc_err}");
		}
		if pandoc_out.len() > 0 {
			println!("{pandoc_out}");
		}
	} else {
		let e_msg: String = pandoc_invoke
			.stderr
			.try_into()
			.unwrap_or_else(|_| "unknown error calling pandoc".into());
		return Err(Error::PandocOutput(e_msg.into_boxed_str()));
	}
	Ok(())
}

#[derive(Debug, Deserialize)]
pub struct Metadata {
	pub departments: Vec<Box<str>>,
	pub authors: Vec<Box<str>>,
	pub status: DocStatus,
}

#[derive(Debug, Deserialize, strum::Display, Clone, Copy)]
pub enum DocStatus {
	Draft,
	Issued,
	Depreciated,
}

/// Expects clear file_buf and clears file buffer after use.
///
/// # Errors
///
/// This function will return an error if .
pub fn parse_toml(file_buf: &mut String, toml_path: &Path) -> Result<Metadata, Error> {
	let mut toml_handle = OpenOptions::new()
		.read(true)
		.write(false)
		.create(false)
		.open(toml_path)
		.map_err(|ioe| Error::OpenMetadata {
			ioe,
			path: Box::from(toml_path),
		})?;

	toml_handle
		.read_to_string(file_buf)
		.map_err(|ioe| Error::ReadMetadata {
			ioe,
			path: Box::from(toml_path),
		})?;
	let res = toml::from_str(&file_buf).map_err(|tpe| Error::ParseMetadata {
		tpem: tpe.message().into(),
		path: Box::from(toml_path),
	});
	file_buf.clear();
	res
}

pub fn compile_yaml_metadata(
	mut file_buf: String,
	metadata: &Metadata,
	parsed_name: &ParsedMdName,
	time: &DateTime<Local>,
) -> String {
	use std::fmt::Write as _;
	let b = &mut file_buf;

	writeln!(b, "---").unwrap();
	let code = parsed_name.code;
	let number = parsed_name.number;
	let title = &parsed_name.title;
	writeln!(b, "title: {code}-{number} {title}").unwrap();
	writeln!(b, "author:").unwrap();
	for department in &metadata.departments {
		writeln!(b, "- {department}").unwrap()
	}
	let build_date = time.format("%Y/%m/%d UTC %z");
	writeln!(b, "date: {build_date}").unwrap();
	writeln!(b, "fontsize: 12pt").unwrap();
	writeln!(b, "toc: true").unwrap();
	writeln!(b, "mainfont: AtkinsonHyperlegibleNext").unwrap();
	// writeln!(b, "fontfamily: AtkensonHyperlegibleNext").unwrap();
	writeln!(b, "papersize: letter").unwrap();
	// extra NL so we leave a blank line before the first section
	writeln!(b, "---\n").unwrap();
	file_buf
}

pub fn compile_std_sections(
	mut file_buf: String,
	metadata: &Metadata,
	parsed_name: &ParsedMdName,
	revision: usize,
	hash: u64,
	len: usize,
	time: &DateTime<Local>,
) -> String {
	use std::fmt::Write as _;
	let b = &mut file_buf;
	let code = parsed_name.code;
	let number = parsed_name.number;
	let status = metadata.status;
	let build_time = time.format("%+");
	writeln!(b, "# Document Control {{-}}\n").unwrap();
	writeln!(b, "**Document Type:** {code}\n").unwrap();
	writeln!(b, "**Document #:** {number}\n").unwrap();
	writeln!(b, "**Status:** {status}\n").unwrap();
	writeln!(b, "**Version:** {revision}\n").unwrap();
	writeln!(
		b,
		"**Source File Hash [XXH3 64](https://xxhash.com/):** {hash:X}\n"
	)
	.unwrap();
	writeln!(b, "**Source File Length:** {len}\n").unwrap();
	writeln!(b, "**Build Timestamp:** {build_time}\n").unwrap();
	writeln!(b, "# Authors {{-}}\n").unwrap();
	for author in &metadata.authors {
		writeln!(b, "* {author}").unwrap();
	}
	b.write_char('\n').unwrap();
	file_buf
}
