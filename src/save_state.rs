#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SaveState {
	pub cc:                u8,
	pub active_func:       usize,
	pub is_fft:            bool,
	pub current_intensity: f32,
	pub time_dialation:    f32,
	pub decay_factor:      f32,
	pub lum_mod:           f32,
	pub modulo_param:      f32,
	pub decay_param:       f32,
}

impl SaveState {
	pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self, Box<dyn std::error::Error>> {
		let file = std::fs::read_to_string(path.as_ref())?;
		Ok(toml::from_str(&file)?)
	}

	pub fn from_dir(path: impl AsRef<std::path::Path>) -> Vec<(String, Vec<SaveState>)> {
		let Ok(dir) = std::fs::read_dir(path.as_ref()) else { return Vec::new(); };

		dir.filter_map(|entry| {
				let path = entry.ok()?.path();
				path.is_dir().then_some(path)
			})
			.filter_map(|path| {
				let paths = std::fs::read_dir(&path).ok()?
					.filter_map(|entry| {
						let path = entry.ok()?.path();
						path.is_file().then_some(path)
					})
					.filter_map(|path| std::fs::read_to_string(&path).ok())
					.map(|file| toml::from_str::<SaveState>(&file))
					.collect::<Result<Vec<_>, toml::de::Error>>() 
					.unwrap_or_else(|e| {
						eprintln!("[MAIN]: Error reading save_state file: {e}");
						std::process::exit(1);
					});

				Some((String::from(path.file_name()?.to_str()?), paths))
			})
			.collect::<Vec<_>>()
	}
}
