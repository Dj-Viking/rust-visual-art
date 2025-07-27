use std::sync::{MutexGuard};
pub fn handle_save_preset(ms: &mut MutexGuard<crate::MutState>) -> () {
	if ms.is_listening && ms.is_saving_preset {
		let ss = crate::SaveState::new(ms);

		// TODO: if the CC was different than the current one then save a new file
		// instead of saving over the existing one
		// should the save states be committed since they are user defined and can change
		// frequently?
		let mut tomlstring: String = String::from(format!("[{}]", { 
			ms.save_state.cc 
		}));

		let toml = toml::to_string(&ss).unwrap();

		tomlstring.push_str(&*Box::leak(format!("\n{}", toml).into_boxed_str()));

		if ms.save_state.cc != "0".to_string().parse::<u8>().unwrap() 
			&& unsafe { crate::USER_SS_ON }
		{
			// make user ss folder if not exist
			if let Ok(_) = std::fs::read_dir("user_ss_config") {
				println!("user ss folder exists\n\tsaving new preset on cc {:?}", ms.save_state.cc);
				let _ = std::fs::write(
					format!("user_ss_config/{}_save_state.toml", ms.save_state.cc), 
					tomlstring
				);
			} else {
				println!("user ss folder did not exist\n\tsaving new preset on cc {:?}", ms.save_state.cc);
				let _ = std::fs::create_dir("user_ss_config");
				let _ = std::fs::write(
					format!("user_ss_config/{}_save_state.toml", ms.save_state.cc), 
					tomlstring
				);
			}
		} else { // save to the default state file
			// TODO: rename to default_user_save_state
			let _ = std::fs::write("save_states.toml", tomlstring);
		}

		println!("is_listening false");
		ms.is_listening = false;
	}
}
