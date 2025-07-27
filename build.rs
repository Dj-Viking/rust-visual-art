fn main() {

	const LIBS_PATH: &str = "target/libs";

	if let Ok(entries) = std::fs::read_dir(LIBS_PATH) {
		for entry in entries {
			if let Ok(entry) = entry {
				println!("remove {:?}", entry.path());
				std::fs::remove_file(entry.path()).unwrap();
			}
		}
	}
	
	std::fs::read_dir("lib").unwrap()
		.filter_map(Result::ok)
		.filter(|entry| entry.file_type().unwrap().is_file())
		.for_each(|entry| handle_compile_dylib(&entry));
}
fn handle_compile_dylib(entry: &std::fs::DirEntry) -> () {
	let path = entry.path();
	
	let status = std::process::Command::new("rustc")
		.args(["--crate-type=dylib", "-o", 
			&("target/libs/".to_string() + path.file_stem().unwrap().to_str().unwrap()),
			path.to_str().unwrap()])
		.output()
		.unwrap();

	if !status.status.success() {
		std::fs::create_dir("target/libs/").unwrap();
		return handle_compile_dylib(entry);
	} else {
		println!("{}", std::str::from_utf8(&status.stdout).unwrap());
		println!("{}", std::str::from_utf8(&status.stderr).unwrap());
		assert!(status.status.success());
	}
}
