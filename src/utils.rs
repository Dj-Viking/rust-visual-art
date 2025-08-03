use std::sync::{MutexGuard};
use portmidi::PortMidi;
use std::collections::HashMap;

// returns the midiccs needed for main () entrypoint
// i know im reading the file twice but oh well
// i can't as easily pass mutstate around due to borrow-checker ownership rules
pub fn get_midi_ccs(pm_ctx: &portmidi::PortMidi) -> Result<Vec<u8>, Box<dyn std::error::Error>> {

	let devices = pm_ctx.devices()?;

	let config: HashMap<String, crate::midi::DeviceConfig> = 
		toml::from_str(&std::fs::read_to_string(*crate::CONF_FILE).unwrap()).unwrap_or_else(|e| {
			eprintln!("[MIDI]: Error reading config file: {e}");
			std::process::exit(1);
		});

	// TODO: rename config.toml to midi_config.toml
	println!("loaded midi config.toml {:#?}", config);

	// this finds the device of the first controller it finds in the list 
	// which is connected to the computer but this only allows one controller
	// to be used at a time when using the app
	//
	// TODO: how can we have multiple controllers at once?
	let dev = devices.into_iter()
		.find(|d| 
			d.direction() == portmidi::Direction::Input 
			&& config.keys().any(|n| n == d.name()))
		.unwrap_or_else(|| {
			eprintln!("[MIDI]: No device defined in config found - \ndid you plug in a MIDI controller yet?\nOr have you configured your controller for the config.toml file yet?");
			std::process::exit(1);
		});

	let mut midi_ccs = config[dev.name()].fns.clone().to_vec();

	for key in config[dev.name()].keys() {
		if let Some(value) = config[dev.name()].get(key) {
			midi_ccs.push(value as u8);
		}
	}

	Ok(midi_ccs)
}

pub fn handle_save_preset_midi(ms: &mut MutexGuard<crate::MutState>) -> () {
	if ms.is_saving_preset && ms.is_listening_midi {
		let ss = crate::SaveState::new(ms);

		let mut tomlstring: String = String::from(format!("[{}]", { 
			ms.save_state.cc 
		}));

		let ss_ = ss.clone();
		let toml = toml::to_string(&ss_).unwrap();

		// place into the existing ms.user_cc_map
		let ms_ss_cc = ms.save_state.cc.clone();
		let ss_ = ss.clone();
		ms.user_cc_map.insert(format!("{}", ms_ss_cc), ss_);

		tomlstring.push_str(&*Box::leak(format!("\n{}", toml).into_boxed_str()));

		println!("[UTILS][MIDI]: what is cc here in save midi {}", ms.save_state.cc);

		// if we're not saving default it's for midi cc map
		if ms.save_state.cc != "0".to_string().parse::<u8>().unwrap() 
		{
			// make user ss folder if not exist
			if let Ok(_) = std::fs::read_dir("user_ss_config") {
				println!("[UTILS][KEYS]: user ss folder exists\n\tsaving new preset on cc {:?}", ms.save_state.cc);
				let _ = std::fs::write(
					format!("user_ss_config/{}_save_state.toml", ms.save_state.cc), 
					tomlstring
				);
			} else {
				println!("[UTILS][KEYS]: user ss folder did not exist\n\tsaving new preset on cc {:?}", ms.save_state.cc);
				let _ = std::fs::create_dir("user_ss_config");
				let _ = std::fs::write(
					format!(
						"user_ss_config/{}_save_state.toml", 
						ms.save_state.cc), 
					tomlstring
				);
			}
		}

		println!("[UTILS][KEYS]: is_listening_midi false");
		ms.is_listening_midi = false;
	}

}
// output a number within a specific range from an entirely
pub fn lerp_float(
    input:  u8,  // - input value to determine what position in the range the number sits
    minout: f32, // - minimum percentage value
    maxout: f32, // - maximum percentage value
    minin:  u8,  // - minimum input value the range can be
    maxin:  u8,  // - maximum input value the range can be
) -> f32 {
	(maxout - minout) * ((input - minin) as f32)
	   / ((maxin - minin) as f32) + minout
}
pub fn use_default_user_save_state(ss_map: &HashMap<String, crate::SaveState>) -> Option<crate::SaveState> {
	if unsafe { !crate::HMR_ON } {
		return Some(crate::SaveState {
			cc:                ss_map["0"].cc,
			active_func:       ss_map["0"].active_func,
			is_fft:            ss_map["0"].is_fft,
			current_intensity: ss_map["0"].current_intensity,
			time_dialation:    ss_map["0"].time_dialation,
			decay_factor:      ss_map["0"].decay_factor,
			lum_mod:           ss_map["0"].lum_mod,
			modulo_param:      ss_map["0"].modulo_param,
			decay_param:       ss_map["0"].decay_param,
		});
	}

	None
}
pub fn use_user_defined_cc_mappings () 
	-> Result<
		(HashMap<String, crate::SaveState>, Vec<u8>), 
		Box<dyn std::error::Error>>
{
	let mut hm = HashMap::<String, crate::SaveState>::new();
	// read dir for all files
	// parse all files into a hashmap with hashkeys are the cc number for the mapping
	// and the values are the SaveState structs
	
	std::fs::read_dir(*crate::USER_SS_CONFIG)
		// I'll get more than one path here if there's more
		// so then more key values would be inserted to this hashmap
		?.into_iter().for_each(|path| {
			//parse the toml at the dir path	
			let config: HashMap<String, crate::SaveState> = 
				toml::from_str(
					&std::fs::read_to_string(path.unwrap().path()).unwrap()
				).unwrap_or_else(|e| {
					eprintln!("[MAIN]: Error reading save_state file: {e}");
					std::process::exit(1);
				});
			
			hm.insert(
				config.keys().last().unwrap().to_string(),
				config.values().last().unwrap().clone()
			);
		});

	println!("[MAIN]: user ss config cc map {:#?}", hm);

	let hmkeys = hm
			.keys().into_iter()
			.map(|k| k.parse::<u8>().unwrap())
			.collect::<Vec<u8>>();

	Ok((hm, hmkeys))
}
pub fn handle_save_preset_keys(ms: &mut MutexGuard<crate::MutState>) -> () {
	if ms.is_saving_preset && ms.is_listening_keys {
		let mut ss = crate::SaveState::new(ms);

		ss.cc = 0;

		let mut tomlstring: String = String::from(format!("[{}]", { "0" }));

		let toml = toml::to_string(&ss).unwrap();

		tomlstring.push_str(&*Box::leak(format!("\n{}", toml).into_boxed_str()));

		println!("[UTILS][KEYS]: what is cc here in save keys {:?}", "0");

		// save to the default state file
		// use this one for the is_listening_keys control
		let _ = std::fs::write("default_user_save_state.toml", tomlstring);

		println!("[UTILS][KEYS]: is_listening_keys false");
		ms.is_listening_keys = false;
	}
}
