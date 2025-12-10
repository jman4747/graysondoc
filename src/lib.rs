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

pub const TYPST_TEMPLATE: &str = include_str!("./template.typ");
pub const TYPST_TEMPLATE_LEN: u64 = TYPST_TEMPLATE.len() as u64;
pub const TYPST_MEMO_TEMPLATE: &str = include_str!("./memo_template.typ");
pub const TYPST_MEMO_TEMPLATE_LEN: u64 = TYPST_MEMO_TEMPLATE.len() as u64;

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
	#[error("typst executable not available:\n{0:?}")]
	TypstVersionCmd(#[source] std::io::Error),
	#[error("typst executable not available:\n{0:?}")]
	TypstVersionOutput(Box<str>),
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
	#[error("typst executable not available:\n{0:?}")]
	TypstInvoke(std::io::Error),
	#[error("typst executable not available:\n{0}")]
	TypstOutput(Box<str>),
}

pub const METADATA_FILE_NAME: &str = "metadata.toml";

#[cfg(not(target_os = "windows"))]
pub const PANDOC_CMD: &str = "pandoc";
#[cfg(target_os = "windows")]
pub const PANDOC_CMD: &str = "pandoc.exe";
// #[cfg(not(target_os = "windows"))]
// pub const LATEX_CMD: &str = "lualatex";
// #[cfg(target_os = "windows")]
// pub const LATEX_CMD: &str = "lualatex.exe";

#[cfg(not(target_os = "windows"))]
pub const TYPST_CMD: &str = "typst";
#[cfg(target_os = "windows")]
pub const TYPST_CMD: &str = "typst.exe";

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
	/// don't delete typst itermediate representation
	#[argh(switch)]
	pub preserve_ir: bool,
	/// stop after building typst itermediate representation, implies --preserve-ir
	#[argh(switch)]
	pub ir_only: bool,
	/// compile the document as a memo given comma separated author list
	#[argh(option)]
	pub memo: Option<String>,
}

#[derive(Debug)]
pub struct ParsedMdName<'name> {
	pub code: &'name str,
	pub number: usize,
	pub title: Cow<'name, str>,
}

pub fn title_underscore_to_space(raw_title: &str) -> Cow<'_, str> {
	// remove underscores
	let raw_trim_title = raw_title.trim().trim_matches(&['_', '＿']);

	if raw_trim_title.contains('_') {
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
	}
}

pub fn nom_document_name(input: &str) -> IResult<&str, ParsedMdName<'_>> {
	// get the code (doc type) and number
	let mut code_parser = separated_pair(alpha1, char('-'), usize);
	let (rst, (code, number)) = code_parser.parse(input)?;
	let (raw_title, _underscore) = char('_').parse(rst)?;

	// remove underscores
	let title: Cow<'_, str> = title_underscore_to_space(raw_title);

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

	let typst_version = Command::new(TYPST_CMD)
		.arg("--version")
		.output()
		.map_err(|ioe| Error::TypstVersionCmd(ioe))?;

	if typst_version.status.success() {
		const ERR_MSG: &str = "can't read stdout stream from typst";
		let version_msg: String = typst_version
			.stdout
			.try_into()
			.map_err(|_| Error::TypstVersionOutput(Box::from(ERR_MSG)))?;
		let first_line = version_msg
			.lines()
			.next()
			.ok_or_else(|| Error::TypstVersionOutput(Box::from(ERR_MSG)))?;
		println!("With: {first_line}");
	} else {
		let e_msg: String = typst_version
			.stderr
			.try_into()
			.unwrap_or_else(|_| "unknown error getting typst version".into());
		return Err(Error::TypstVersionOutput(e_msg.into_boxed_str()));
	}
	Ok(())
}

/// Markdown to Typst
pub fn call_pandoc(md_path: impl AsRef<OsStr>, ouput_path: impl AsRef<OsStr>) -> Result<(), Error> {
	// pandoc $md -f markdown -t typst -o $output.typ
	let pandoc_invoke = Command::new(PANDOC_CMD)
		.arg(md_path)
		.arg("-f")
		.arg("markdown")
		.arg("-t")
		.arg("typst")
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

/// Typst IR to PDF
pub fn call_typst(
	irtyp_path: impl AsRef<OsStr>,
	ouput_path: impl AsRef<OsStr>,
) -> Result<(), Error> {
	// typst compile $irtyp_path -f pdf
	let typst_invoke = Command::new(TYPST_CMD)
		.arg("compile")
		.arg(irtyp_path)
		.arg("-f")
		.arg("pdf")
		.arg(ouput_path)
		.output()
		.map_err(|ioe| Error::TypstInvoke(ioe))
		.inspect_err(|e| eprintln!("{e}"))?;

	if typst_invoke.status.success() {
		const ERR_MSG: &str = "can't read stdout stream from typst";
		let pandoc_out: String = typst_invoke
			.stdout
			.try_into()
			.map_err(|_| Error::TypstOutput(Box::from(ERR_MSG)))?;
		let pandoc_err: String = typst_invoke
			.stderr
			.try_into()
			.map_err(|_| Error::TypstOutput(Box::from(ERR_MSG)))?;
		if pandoc_err.len() > 0 || pandoc_out.len() > 0 {
			println!("Typst Output:");
		}
		if pandoc_err.len() > 0 {
			println!("{pandoc_err}");
		}
		if pandoc_out.len() > 0 {
			println!("{pandoc_out}");
		}
	} else {
		let e_msg: String = typst_invoke
			.stderr
			.try_into()
			.unwrap_or_else(|_| "unknown error calling typst".into());
		return Err(Error::TypstOutput(e_msg.into_boxed_str()));
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

/// Prepend the metadata to intermediate typst file
pub fn compile_typst_metadata(
	mut file_buf: String,
	metadata: &Metadata,
	parsed_name: &ParsedMdName,
	time: &DateTime<Local>,
	version: usize,
	hash: u64,
	src_len: usize,
) -> String {
	use std::fmt::Write as _;
	let b = &mut file_buf;
	const GD_VERSION: &str = env!("CARGO_PKG_VERSION");
	writeln!(b, "{TYPST_TEMPLATE}").unwrap();

	// #show: project.with(
	// 	title: "Grayson Pandoc",
	// 	authors: (
	// 		"Josh T.",
	// 		"Abcd E."
	// 	),
	// 	departments: (
	// 		"IT",
	// 		"Operations",
	// 		"Outreach"
	// 	),
	// 	doc_type: "SDR",
	// 	document_number: 0,
	// 	date: "2025/11/20 UTC -0500",
	// 	status: "DRAFT",
	// 	version: 0,
	// 	hash: "FFFF",
	// 	src_length: 999,
	// 	graysondoc_version: 0,
	// )
	let title = &parsed_name.title;
	let code = parsed_name.code;
	let number = parsed_name.number;
	let build_date = time.format("%Y/%m/%d UTC %z");
	let status = metadata.status;
	writeln!(b, "#show: project.with(").unwrap();
	writeln!(b, "\ttitle:\"{title}\",").unwrap();

	writeln!(b, "\tauthors: (").unwrap();
	let mut authors = metadata.authors.iter();
	while let Some(author) = authors.next() {
		writeln!(b, "\t\t\"{author}\",").unwrap();
	}
	writeln!(b, "\t),").unwrap();

	let mut departments = metadata.departments.iter();
	writeln!(b, "\tdepartments: (").unwrap();
	while let Some(dept) = departments.next() {
		writeln!(b, "\t\t\"{dept}\",").unwrap();
	}
	writeln!(b, "\t),").unwrap();

	writeln!(b, "\tdoc_type: \"{code}\",").unwrap();
	writeln!(b, "\tdocument_number: {number},").unwrap();
	writeln!(b, "\tdate: \"{build_date}\",").unwrap();
	writeln!(b, "\tstatus: \"{status}\",").unwrap();
	writeln!(b, "\tversion: {version},").unwrap();
	writeln!(b, "\thash: \"{hash:X}\",").unwrap();
	writeln!(b, "\tsrc_length: {src_len},").unwrap();
	writeln!(b, "\tgraysondoc_version: \"{GD_VERSION}\",").unwrap();

	// extra NL so we leave a blank line before the first section
	writeln!(b, ")\n").unwrap();
	file_buf
}

pub fn compile_memo_typst_metadata(
	mut file_buf: String,
	title: &str,
	version: usize,
	hash: u64,
	src_len: usize,
	authors: &str,
	time: &DateTime<Local>,
) -> String {
	use std::fmt::Write as _;
	let b = &mut file_buf;
	const GD_VERSION: &str = env!("CARGO_PKG_VERSION");
	writeln!(b, "{TYPST_MEMO_TEMPLATE}").unwrap();

	// #show: project.with(
	// 	title: "Grayson Pandoc",
	// 	authors: (
	// 		"Josh T.",
	// 		"Abcd E."
	// 	),
	// 	date: "2025/11/20 UTC -0500",
	// 	version: 0,
	// 	hash: "FFFF",
	// 	src_length: 999,
	// 	graysondoc_version: 0,
	// )

	let build_date = time.format("%Y/%m/%d UTC %z");
	writeln!(b, "#show: project.with(").unwrap();
	writeln!(b, "\ttitle:\"{title}\",").unwrap();

	writeln!(b, "\tauthors: (").unwrap();
	let mut authors = authors.split(",").map(|author| author.trim());
	while let Some(author) = authors.next() {
		writeln!(b, "\t\t\"{author}\",").unwrap();
	}
	writeln!(b, "\t),").unwrap();

	writeln!(b, "\tdate: \"{build_date}\",").unwrap();
	writeln!(b, "\tversion: {version},").unwrap();
	writeln!(b, "\thash: \"{hash:X}\",").unwrap();
	writeln!(b, "\tsrc_length: {src_len},").unwrap();
	writeln!(b, "\tgraysondoc_version: \"{GD_VERSION}\",").unwrap();

	// extra NL so we leave a blank line before the first section
	writeln!(b, ")\n").unwrap();
	file_buf
}

// TODO: memos
// pub fn compile_memo_std_sections(
// 	mut file_buf: String,
// 	time: &DateTime<Local>,
// 	authors: &str,
// ) -> String {
// 	use std::fmt::Write as _;
// 	let b = &mut file_buf;
// 	let build_time = time.format("%+");
// 	writeln!(b, "# Document Control {{-}}\n").unwrap();
// 	writeln!(b, "**Version:** {revision}\n").unwrap();
// 	writeln!(
// 		b,
// 		"**Source File Hash [XXH3 64](https://xxhash.com/):** {hash:X}\n"
// 	)
// 	.unwrap();
// 	writeln!(b, "**Source File Length:** {len}\n").unwrap();
// 	writeln!(b, "**Build Timestamp:** {build_time}\n").unwrap();
// 	writeln!(b, "# Authors {{-}}\n").unwrap();
// 	for author in authors.split(",") {
// 		writeln!(b, "* {}", author.trim()).unwrap();
// 	}
// 	b.write_char('\n').unwrap();
// 	file_buf
// }
