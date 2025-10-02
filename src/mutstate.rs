use std::path::Path;
use crate::save_state::SaveState;

#[derive(Default, Debug)]
pub struct MutState {
	pub is_backwards:       bool,
	pub is_reset:           bool,
	pub is_saving_preset:   bool,
	pub is_listening_midi:  bool,
	pub is_listening_keys:  bool,
	pub plugins:            Vec<crate::loading::Plugin>,

	pub controller_name:    String,
	pub save_state:         SaveState,
	pub preset_map:         Vec<(String, Vec<SaveState>)>,
}

impl MutState {
	pub fn save_preset(&mut self) -> Result<(), toml::ser::Error> {
		if !self.is_saving_preset { return Ok(()); }

		let _ = std::fs::create_dir(&*crate::PRESETS_DIR);


		if self.is_listening_midi {
			match self.preset_map.iter_mut().find(|(c, _)| c == &self.controller_name) {
				Some((_, presets)) => presets.push(self.save_state.clone()),
				None => self.preset_map.push((self.controller_name.clone(), vec![self.save_state.clone()])),
			}

			if self.save_state.cc != 0 {
				let controller_dir_path = Path::new(&*crate::PRESETS_DIR).join(&self.controller_name);
				let _ = std::fs::create_dir(&controller_dir_path);

				let toml = toml::to_string(&self.save_state)?;
				std::fs::write(controller_dir_path.join(format!("{}.toml", self.save_state.cc)), toml).unwrap();
			}

			self.is_listening_midi = false;
		}

		if self.is_listening_keys {
			let toml = toml::to_string(&self.save_state)?;
			std::fs::write(Path::new(&*crate::PRESETS_DIR).join("default.toml"), toml).unwrap();

			self.is_listening_keys = false;
		}

		Ok(())
	}

	pub fn set_active_func(&mut self, afn: usize) {
		println!("[MAIN]: active func {:?}", self.save_state.active_func);

		if self.plugins.len() < afn {
			eprintln!("[MAIN]: plugin {:?} not loaded", afn);
			return;
		}

		self.save_state.active_func = afn;
	}
}
