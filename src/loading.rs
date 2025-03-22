use libloading::{Symbol, Library};

#[derive(Debug)]
pub struct Plugin {
	_lib: Library,
	pub time_divisor: f32,
	pub time_dialation_range: f32,
	pub intensity_range: f32,
	transform: unsafe extern "C" fn(
		x:           f32,                     // x 
		y:           f32,                     // y 
		t:           f32,                     // t 
		// TODO: do something interesting with magnitudes in the function?
		// fft:         *const std::ffi::c_void, // vec
		// fft_len:     usize,                   // vec len
		// fft_buf:     *const std::ffi::c_void, // buf
		// fft_buf_len: usize                    // buf len
	) -> f32
}

impl Plugin {
	pub fn load_dir(path: impl AsRef<std::path::Path>, plugs: &mut Vec<Self>) {
		let mut files =  std::fs::read_dir(path).unwrap()
			.filter_map(Result::ok)
			.filter(|entry| entry.file_type().unwrap().is_file())
			.map(|e| e.path())
			.collect::<Vec<_>>();

		files.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

		plugs.extend(
			files.iter()
				.map(|file| unsafe { Library::new(file).unwrap() })
				.map(|lib| Self {
					transform:            *unsafe { lib.get(b"transform").unwrap() },
					time_divisor:          unsafe { lib.get(b"TIME_DIVISOR").map_or(1000000000.0, |s: Symbol<*const f32>| **s) },
					time_dialation_range:  unsafe { lib.get(b"TIME_DIALATION_RANGE").map_or(100.0, |s: Symbol<*const f32>| **s) },
					intensity_range:       unsafe { lib.get(b"INTENSITY_RANGE").map_or(0.01, |s: Symbol<*const f32>| **s) },
					_lib: lib,
				}));
	}

	pub fn call(&self, 
		x: f32, y: f32, t: f32, 
		// fft:     &[(f32, f32)], 
		// fft_buf: &[f32; 69]
	) -> f32 {
		unsafe { (self.transform)(
			x, y, t, 
			// fft.as_ptr() as *const std::ffi::c_void, 
			// fft.len(),
			// fft_buf.as_ptr() as *const std::ffi::c_void,
			// fft_buf.len()
		) }
	}
}
