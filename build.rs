use std::path::PathBuf;

const LIBS_PATH: &str = "target/libs";

fn main() {
	std::fs::read_dir(LIBS_PATH).unwrap()
		.filter_map(|entry| {
			let path = entry.ok()?.path();
			path.is_file().then_some(path)
		})
		.for_each(|path| std::fs::remove_file(path).unwrap());
	
	std::fs::read_dir("lib").unwrap()
		.filter_map(Result::ok)
		.filter(|entry| entry.file_type().unwrap().is_file())
		.for_each(|entry| handle_compile_dylib(&entry));
}

fn handle_compile_dylib(entry: &std::fs::DirEntry) {
	let _ = std::fs::create_dir(LIBS_PATH);

	let path = entry.path();

	let libs_path = PathBuf::from(LIBS_PATH).join(path.file_name().unwrap());
	
	let status = std::process::Command::new("rustc")
		.args(["--crate-type=dylib", "-o"])
		.args([&libs_path, &path])
		.output()
		.unwrap();

	println!("{}", std::str::from_utf8(&status.stdout).unwrap());
	println!("{}", std::str::from_utf8(&status.stderr).unwrap());
	assert!(status.status.success());
}
