pub struct Plugin {
	_lib: libloading::Library,
	transform: unsafe extern "C" fn(f32, f32, f32, *const std::ffi::c_void, freq_len: usize) -> f32,
}

impl Plugin {
	pub fn load_dir(path: impl AsRef<std::path::Path>) -> Vec<Self> {
		let mut files =  std::fs::read_dir(path).unwrap()
			.filter_map(Result::ok)
			.filter(|entry| entry.file_type().unwrap().is_file())
			.map(|e| e.path())
			.collect::<Vec<_>>();

		files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

		files.iter()
			.map(|file| unsafe { libloading::Library::new(file).unwrap() })
			.map(|lib| Self {
				transform: *unsafe { lib.get(b"transform").unwrap() },
				_lib: lib,
			})
			.collect()
	}

	pub fn call(&self, x: f32, y: f32, t: f32, fft: &[(f32, f32)]) -> f32 {
		unsafe { (self.transform)(x, y, t, fft.as_ptr() as *const std::ffi::c_void, fft.len()) }
	}
}
