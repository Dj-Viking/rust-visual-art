fn main() {
	let _ = std::fs::remove_dir_all("target/libs");
	std::fs::create_dir_all("target/libs").unwrap();
	
	std::fs::read_dir("lib").unwrap()
		.filter_map(Result::ok)
		.filter(|entry| entry.file_type().unwrap().is_file())
		.for_each(|entry| {
			let path = entry.path();
			
			let status = std::process::Command::new("rustc")
				.args(["--crate-type=dylib", "-o", 
					&("target/libs/".to_string() + path.file_stem().unwrap().to_str().unwrap()),
					path.to_str().unwrap()])
				.output()
				.unwrap();

			println!("{}", std::str::from_utf8(&status.stdout).unwrap());
			println!("{}", std::str::from_utf8(&status.stderr).unwrap());
			assert!(status.status.success());
		});
}
