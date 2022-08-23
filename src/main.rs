use clap::{Parser};
use clue::{flag, check, compiler::*, parser::*, scanner::*, ENV_DATA, env::{ContinueMode, TypesMode}};
use std::{ffi::OsStr, fmt::Display, fs, fs::File, io::prelude::*, path::Path, time::Instant};

#[derive(Parser)]
#[clap(about = "C/Rust like programming language that compiles into Lua code\nMade by Maiori\nhttps://github.com/ClueLang/Clue", version, long_about = None)]
struct Cli {
	/// The path to the directory where the *.clue files are located.
	/// Every directory inside the given directory will be checked too.
	/// If the path points to a single *.clue file, only that file will be compiled.
	#[clap(required_unless_present = "license")]
	path: Option<String>,

	/// The name the output file will have
	#[clap(default_value = "main", value_name = "OUTPUT FILE NAME")]
	outputname: String,

	/// Print license information
	#[clap(short = 'L', long, display_order = 1000)]
	license: bool,

	/// Print list of detected tokens in compiled files
	#[clap(long)]
	tokens: bool,

	/// Print syntax structure of the tokens of the compiled files
	#[clap(long)]
	r#struct: bool,

	/// Print output Lua code in the console
	#[clap(long)]
	output: bool,

	/// Use LuaJIT's bit library for bitwise operations
	#[clap(short, long, value_name = "VAR NAME")]
	jitbit: Option<String>,

	/// Change the way continue identifiers are compiled
	#[clap(short, long, value_enum, default_value = "simple", value_name = "MODE")]
	r#continue: ContinueMode,

	/// Don't save compiled code
	#[clap(short = 'D', long)]
	dontsave: bool,

	/// Treat PATH not as a path but as Clue code
	#[clap(short, long)]
	pathiscode: bool,

	/// Use rawset to create globals
	#[clap(short, long)]
	rawsetglobals: bool,

	/// Include debug comments in the output
	#[clap(short, long)]
	debugcomments: bool,

	/// Enable type checking (might slow down compilation)
	#[clap(short = 'T', long, value_enum, default_value = "none", value_name = "MODE")]
	types: TypesMode
}

fn add_to_output(string: &str) {
	ENV_DATA
		.write()
		.expect("Can't lock env_data")
		.add_output_code(String::from(string));
}

fn compile_code(code: String, name: String, scope: usize) -> Result<String, String> {
	let time = Instant::now();
	let tokens: Vec<Token> = scan_code(code, name.clone())?;
	if flag!(env_tokens) {
		println!("Scanned tokens of file \"{}\":\n{:#?}", name, tokens);
	}
	let ctokens = parse_tokens(tokens, name.clone())?;
	if flag!(env_struct) {
		println!("Parsed structure of file \"{}\":\n{:#?}", name, ctokens);
	}
	let code = compile_tokens(scope, ctokens);
	if flag!(env_output) {
		println!("Compiled Lua code of file \"{}\":\n{}", name, code);
	}
	println!(
		"Compiled file \"{}\" in {} seconds!",
		name,
		time.elapsed().as_secs_f32()
	);
	Ok(code)
}

fn compile_file<P: AsRef<Path>>(path: P, name: String, scope: usize) -> Result<String, String>
where
	P: AsRef<OsStr> + Display,
{
	let mut code: String = String::new();
	check!(check!(File::open(path)).read_to_string(&mut code));
	compile_code(code, name, scope)
}

fn compile_folder<P: AsRef<Path>>(path: P, rpath: String) -> Result<(), String>
where
	P: AsRef<OsStr> + Display,
{
	for entry in check!(fs::read_dir(&path)) {
		let entry = check!(entry);
		let name: String = entry
			.path()
			.file_name()
			.unwrap()
			.to_string_lossy()
			.into_owned();
		let filepath_name: String = format!("{}/{}", path, name);
		let filepath: &Path = Path::new(&filepath_name);
		let rname = rpath.clone() + &name;
		if filepath.is_dir() {
			compile_folder(filepath_name, rname + ".")?;
		} else if filepath_name.ends_with(".clue") {
			let code = compile_file(filepath_name, name, 2)?;
			let rname = rname.strip_suffix(".clue").unwrap();
			add_to_output(&format!(
				"[\"{}\"] = function()\n{}\n\tend,\n\t",
				rname, code
			));
		}
	}
	Ok(())
}

fn main() -> Result<(), String> {
	let cli = Cli::parse();
	if cli.license {
		println!("{}", include_str!("../LICENSE"));
		return Ok(());
	}
	ENV_DATA.write().expect("Can't lock env_data").set_data(
		cli.tokens,
		cli.r#struct,
		cli.output,
		cli.jitbit,
		cli.r#continue,
		cli.dontsave,
		cli.pathiscode,
		cli.rawsetglobals,
		cli.debugcomments,
		cli.types,
	);
	if let Some(bit) = &flag!(env_jitbit) {
		add_to_output(&format!("local {} = require(\"bit\");\n", bit));
	}
	let codepath = cli.path.unwrap();
	if flag!(env_pathiscode) {
		let code = compile_code(codepath, String::from("(command line)"), 0)?;
		println!("{}", code);
		return Ok(());
	}
	let path: &Path = Path::new(&codepath);
	if path.is_dir() {
		add_to_output(include_str!("base.lua"));
		compile_folder(&codepath, String::new())?;
		add_to_output("\r}\nimport(\"main\")");
		if !flag!(env_dontsave) {
			let outputname = &format!("{}.lua", cli.outputname);
			let compiledname = if path.display().to_string().ends_with('/')
				|| path.display().to_string().ends_with('\\')
			{
				format!("{}{}", path.display(), outputname)
			} else {
				format!("{}/{}", path.display(), outputname)
			};
			check!(fs::write(
				compiledname,
				ENV_DATA.read().expect("Can't lock env_data").ouput_code()
			))
		}
	} else if path.is_file() {
		let code = compile_file(
			&codepath,
			path.file_name().unwrap().to_string_lossy().into_owned(),
			0,
		)?;
		add_to_output(&code);
		if !flag!(env_dontsave) {
			let compiledname =
				String::from(path.display().to_string().strip_suffix(".clue").unwrap()) + ".lua";
			check!(fs::write(
				compiledname,
				ENV_DATA.read().expect("Can't lock env_data").ouput_code()
			))
		}
	} else {
		return Err(String::from("The given path doesn't exist"));
	}
	Ok(())
}

#[cfg(test)]
mod test {
	use crate::compile_folder;

	#[test]
	fn compilation_success() {
		compile_folder("examples/", String::new()).unwrap();
	}
}
