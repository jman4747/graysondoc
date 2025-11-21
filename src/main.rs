use std::{
	fmt::Write,
	fs::OpenOptions,
	io::{Read, Write as IOW},
	path::PathBuf,
};

use argh::from_env;
use graysondoc::{
	DocStatus, Error, GdocCli, TYPST_TEMPLATE_LEN, call_pandoc, call_typst, check_for_exes,
	compile_typst_metadata, nom_document_name, parse_toml,
};

fn main() -> Result<(), Error> {
	let gdoc: GdocCli = from_env();
	if gdoc.version {
		println!("Graysondoc version: {}", env!("CARGO_PKG_VERSION"));
		return Ok(());
	}
	check_for_exes()?;
	let input_md_path = gdoc.input_md;
	let file_name = if !input_md_path.is_file() {
		if input_md_path.exists() {
			return Err(Error::InputIsFile(input_md_path.into_boxed_path()));
		} else {
			return Err(Error::InputExists(input_md_path.into_boxed_path()));
		}
	} else {
		input_md_path
			.file_name()
			.expect("checked is_file() == true")
			.to_string_lossy()
	};

	println!("Input file: {file_name}");

	// TODO: better architecture for this
	if let Some(_authors) = gdoc.memo {
		println!("Compiling as a memo...");
		println!("Memos not implemented");
		// 	let est_buf_len = {
		// 		let md_est = input_md_path
		// 			.metadata()
		// 			.map(|m| m.len() + 1024) // 1024 for metadata
		// 			.unwrap_or(12 * 1024);
		// 		md_est as usize
		// 	};

		// 	let file_buf = String::with_capacity(est_buf_len);

		// 	let timestamp = chrono::Local::now();

		// 	let buf_w_yaml = compile_memo_yaml_metadata(file_buf, &file_name, &timestamp);
		// 	let input_md_content =
		// 		std::fs::read_to_string(&input_md_path).map_err(|e| Error::ReadInputMd(e))?;
		// 	let irmd_path: PathBuf = {
		// 		let mut input_clone = input_md_path.clone();
		// 		input_clone.set_extension("");
		// 		let mut s = input_clone.as_mut_os_string();
		// 		write!(&mut s, "-{}", gdoc.revision).unwrap();
		// 		input_clone.set_extension("irmd");
		// 		input_clone
		// 	};

		// 	let hash: u64 = xxhash_rust::const_xxh3::xxh3_64(input_md_content.as_bytes());

		// 	let mut buf_w_std_sections = compile_memo_std_sections(
		// 		buf_w_yaml,
		// 		gdoc.revision,
		// 		hash,
		// 		input_md_content.len(),
		// 		&timestamp,
		// 		&authors,
		// 	);

		// 	buf_w_std_sections.write_str(&input_md_content).unwrap();

		// 	// write irmd
		// 	// TODO: use OpenOptions + proper errors
		// 	std::fs::write(&irmd_path, buf_w_std_sections).expect("write all to irmd");
		// 	if gdoc.ir_only {
		// 		println!("Created intermediate mardown at: {:?}", &irmd_path);
		// 		println!("Exiting...")
		// 	} else {
		// 		let output_path: Box<std::ffi::OsStr> = {
		// 			let mut input_clone = irmd_path.clone();
		// 			input_clone.set_extension("pdf");
		// 			input_clone.into_os_string().into_boxed_os_str()
		// 		};

		// 		println!("Building document...");
		// 		call_pandoc(&irmd_path, &output_path)?;
		// 		if !gdoc.preserve_ir {
		// 			let _ = std::fs::remove_file(&irmd_path).inspect_err(|ioe| {
		// 				eprintln!(
		// 					"Unable to remove intermediate markdown at: {:?}\n{ioe}",
		// 					&irmd_path
		// 				)
		// 			});
		// 		} else {
		// 			println!("Preserving intermediate mardown at: {:?}", &irmd_path);
		// 		}
		// 		println!("Document created at: {output_path:?}")
		// 	}
		return Ok(());
	}

	let (before, _after) = file_name
		.rsplit_once(".md")
		.ok_or_else(|| Error::IsMd(input_md_path.clone().into_boxed_path()))
		.inspect_err(|e| eprintln!("{e}"))?;

	let parsed_name = nom_document_name(&before)
		.map_err(|ne| Error::ParseFileName {
			name: Box::from(file_name.as_ref()),
			ne: ne.to_owned(),
		})
		.inspect_err(|e| eprintln!("{e}"))?;

	let title = parsed_name.1.title.as_ref();

	println!("Document code: {}", parsed_name.1.code);
	println!("Document number: {}", parsed_name.1.number);
	println!("Document title: {}", title);

	let toml_path = {
		match gdoc.metadata {
			Some(given) => given,
			None => {
				use std::fmt::Write;
				let mut cloned = input_md_path.clone();
				cloned.pop();
				// we use push so that PathBuf will handle the path separator
				cloned.push(parsed_name.1.code);
				write!(cloned.as_mut_os_string(), "-{}", parsed_name.1.number).unwrap();
				cloned.set_extension("toml");
				cloned
			}
		}
	};

	// check for toml
	println!("Looking for metadata at: {toml_path:?}");
	if !toml_path.is_file() {
		return Err(Error::CheckToml(toml_path.into_boxed_path()));
	}
	let est_buf_len = {
		let md_est = input_md_path
			.metadata()
			.map(|m| m.len() + 256)
			.unwrap_or(12 * 1024);
		let toml_est = toml_path.metadata().map(|m| m.len() + 128).unwrap_or(2048) * 2;
		(md_est + toml_est + TYPST_TEMPLATE_LEN) as usize
	};

	let mut metadata_file_buf = String::with_capacity(est_buf_len);

	let metadata = parse_toml(&mut metadata_file_buf, &toml_path)?;
	let mut file_buf = {
		metadata_file_buf.clear();
		metadata_file_buf
	};

	println!("With metadata:\n{metadata:?}");

	let mut md_file = OpenOptions::new()
		.read(true)
		.write(false)
		.create(false)
		.open(&input_md_path)
		.expect("open Typst IR file");

	let mut buf_w_md = {
		md_file
			.read_to_string(&mut file_buf)
			.map_err(|e| Error::ReadInputMd(e))?;
		file_buf
	};

	drop(md_file);

	let first_line = buf_w_md.lines().next();
	match first_line {
		Some(content) => {
			if content.trim_end() != "# Objectives" {
				return Err(Error::BadFirstSection(Box::from(content)));
			}
		}
		None => return Err(Error::BadFirstSection(Box::from(first_line.unwrap_or("")))),
	}
	let hash: u64 = xxhash_rust::const_xxh3::xxh3_64(buf_w_md.as_bytes());
	let md_len = buf_w_md.len();

	let file_buf = {
		buf_w_md.clear();
		buf_w_md
	};

	let irtyp_path: PathBuf = {
		let mut input_clone = input_md_path.clone();
		input_clone.set_extension("");
		let mut s = input_clone.as_mut_os_string();
		match metadata.status {
			DocStatus::Draft => {
				write!(&mut s, "-DRAFT-{}", gdoc.revision).unwrap();
			}
			DocStatus::Issued => {
				write!(&mut s, "-{}", gdoc.revision).unwrap();
			}
			DocStatus::Depreciated => {
				write!(&mut s, "-DEPRECIATED-{}", gdoc.revision).unwrap();
			}
		}
		input_clone.set_extension("typ");
		input_clone
	};

	// create Typst file
	call_pandoc(&input_md_path, &irtyp_path)?;

	if gdoc.ir_only {
		println!("Created intermediate typst at: {:?}", &irtyp_path);
		println!("Exiting...")
	} else {
		let timestamp = chrono::Local::now();

		let mut buf_w_typst_metadata = compile_typst_metadata(
			file_buf,
			&metadata,
			&parsed_name.1,
			&timestamp,
			gdoc.revision,
			hash,
			md_len,
		);

		// TODO: move to lib + error
		let mut irtyp_file = OpenOptions::new()
			.read(true)
			.write(false)
			.create(false)
			.open(&irtyp_path)
			.expect("open Typst IR file");

		let buf_w_all_typst = {
			irtyp_file
				.read_to_string(&mut buf_w_typst_metadata)
				.expect("read Typst IR file");
			buf_w_typst_metadata
		};

		drop(irtyp_file);

		// write Typst IR file
		// TODO: move to lib + error
		let mut irtyp_file = OpenOptions::new()
			.write(true)
			.create(false)
			.truncate(true)
			.open(&irtyp_path)
			.expect("open Typst IR file");
		irtyp_file
			.write_all(buf_w_all_typst.as_bytes())
			.expect("write all to Typst IR");
		irtyp_file.flush().expect("flush to Typst IR");
		irtyp_file.sync_all().expect("sync Typst IR");
		drop(irtyp_file);

		let output_path: Box<std::ffi::OsStr> = {
			let mut input_clone = irtyp_path.clone();
			input_clone.set_extension("pdf");
			input_clone.into_os_string().into_boxed_os_str()
		};

		println!("Building document...");
		call_typst(&irtyp_path, &output_path)?;
		if !gdoc.preserve_ir {
			let _ = std::fs::remove_file(&irtyp_path).inspect_err(|ioe| {
				eprintln!(
					"Unable to remove intermediate Typst at: {:?}\n{ioe}",
					&irtyp_path
				)
			});
		} else {
			println!("Preserving intermediate Typst at: {:?}", &irtyp_path);
		}
		println!("Document created at: {output_path:?}")
	}
	Ok(())
}
