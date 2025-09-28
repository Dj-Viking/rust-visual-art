use portmidi::PortMidi;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

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
			&& config.keys().any(|n| n == d.name()));

	match dev {
		Some(d) => {
			let mut midi_ccs = config[d.name()].fns.clone().to_vec();

			for key in config[d.name()].keys() {
				if let Some(value) = config[d.name()].get(key) {
					midi_ccs.push(value);
				}
			}

			Ok(midi_ccs)
		},
		None => {
			Err("could not find device".into())
		},
	}
}

pub fn handle_save_preset_midi(ms: &mut MutexGuard<crate::MutState>) -> () {
	if ms.is_saving_preset && ms.is_listening_midi {
		let ss = crate::SaveState::new(ms);
		let controller_name = ms.controller_name.clone();

		let mut tomlstring: String = String::from(format!("[{}]", { 
			ms.save_state.cc 
		}));

		let ss_ = ss.clone();
		let toml = toml::to_string(&ss_).unwrap();

		// place into the existing ms.user_cc_map
		let ms_ss_cc = ms.save_state.cc.clone();
		let ss_ = ss.clone();

		match ms.user_cc_map.get_mut(&controller_name.clone()) 
		{
			Some(val) => {
				val.insert(format!("{}", ms_ss_cc), ss_); 
			},
			None => {
				ms.user_cc_map.insert(controller_name.clone(), HashMap::<String, crate::SaveState>::new());
				ms.user_cc_map.get_mut(&controller_name.clone())
				.unwrap()
				.insert(format!("{}", ms_ss_cc), ss_); 
			},
		}

		// create toml string
		tomlstring.push_str(&*Box::leak(format!("\n{}", toml).into_boxed_str()));

		println!("[UTILS][MIDI]: what is cc here in save midi {}\nfor controllername: {}", ms_ss_cc, controller_name);

		// if we're not saving default it's for midi cc map
		// TODO: make new folder for each controller being used
		// ex user_ss_config/<controller_name>/<cc>_save_state.toml
		if ms.save_state.cc != "0".to_string().parse::<u8>().unwrap() 
		{
			let _ = std::fs::create_dir("user_ss_config");
			let _ = std::fs::create_dir(format!("user_ss_config/{}", controller_name));
			println!("[UTILS][KEYS]: user ss folder [{}] exists \n\tsaving new preset on cc {:?}", 
				controller_name, ms_ss_cc
			);
			let _ = std::fs::write(
				format!("user_ss_config/{}/{}_save_state.toml", controller_name, ms_ss_cc), 
				tomlstring
			);
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

pub fn watch(path: &str, ms_: &std::sync::Arc<Mutex<crate::MutState>>) {
	let (tx, rx) = std::sync::mpsc::channel();

	use notify::Watcher;
	let mut watcher = notify::RecommendedWatcher::new(tx, notify::Config::default()).unwrap();

	// Add a path to be watched. All files and directories at that path and
	// below will be monitored for changes.
	// ....nonrecursive does the same thing as recursive but whatever....
	watcher.watch(path.as_ref(), notify::RecursiveMode::NonRecursive).unwrap();

	let mut event_count = 0;

	for res in rx { match res {
		Ok(event) => {
			if event.kind == notify::event::EventKind::Remove(
				notify::event::RemoveKind::File
			) {

				let lib_name = event.paths[0]
					.to_str().unwrap()
					.split("/")
					.last().unwrap();

				event_count += 1;

				println!("[MAIN]: lib removed: {:?}", lib_name);
				// wait for files to be fully removed
				// and recreated by the build script
				// kind of a weird hack because an event is fired for each File
				// in the plugin path but I need to wait for all the files
				// to be replaced
				if event_count == unsafe { crate::PLUGS_COUNT * 4 } {

					println!("[MAIN]: event count {:?}", event_count);

					let mut ms = ms_.lock().unwrap();

					println!("[MAIN]: \n=========\n watch event: {:?}", event.kind);

					event_count = 0;

					println!("[MAIN]: [INFO]: reloading plugins");
					std::thread::sleep(
						std::time::Duration::from_millis(
							100));
					ms.plugins.clear();
					crate::loading::Plugin::load_dir(*crate::PLUGIN_PATH, &mut ms.plugins);
				}
			}
		},
		Err(error) => println!("[MAIN]: Error: {:?}", error),
	} }
}
// loading presets
pub fn use_user_defined_cc_mappings (controller_name: String)
	-> Result<
		(HashMap<String, HashMap<String, crate::SaveState>>, Vec<u8>),
		Box<dyn std::error::Error>>
{
	let mut hm = HashMap::<String, HashMap<String, crate::SaveState>>::new();
	hm.insert(controller_name.clone(), HashMap::<String, crate::SaveState>::new());
	// read dir for all sub folders which are the controller names
	// and then read that controller_name folder's toml files
	// gather those into the midi_ccs number collection and the hashmap for
	// recalling the state into visibility
	
	// search also through controller_name dir
	std::fs::read_dir(*crate::USER_SS_CONFIG)
		// I'll get more than one path here if there's more
		// so then more key values would be inserted to this hashmap
		?.into_iter().for_each(|direntry| {
			let dirpath = direntry.ok().unwrap().path();
			std::fs::read_dir(dirpath)
				.unwrap().into_iter().for_each(|pathentry| {
					//parse the toml at the dir path
					let config: HashMap<String, crate::SaveState> =
						toml::from_str(
							&std::fs::read_to_string(pathentry.unwrap().path()).unwrap()
						).unwrap_or_else(|e| {
							eprintln!("[MAIN]: Error reading save_state file: {e}");
							std::process::exit(1);
						});

					println!("wtf is happening {:?}", hm);
					
					hm.get_mut(&controller_name.clone()) 
					.unwrap()
					.insert(
						config.keys().last().unwrap().to_string(),
						config.values().last().unwrap().clone()
					); 
					
				})
		});

	println!("[MAIN]: user ss config cc map {:#?}", hm);

	let hmkeys = hm.get(&controller_name.clone()).unwrap()
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
