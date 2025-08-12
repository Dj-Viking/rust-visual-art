use portmidi::{DeviceInfo, MidiEvent};
use std::collections::HashMap;

use std::sync::{MutexGuard};

use crate::MutState;

use serde_json::Value;

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct DeviceConfig {
	pub backwards:         u8,
	pub intensity:         u8,
	pub time_dialation:    u8,
	pub decay_factor:      u8,
	pub lum_mod:           u8,
	pub reset:             u8,
	pub is_fft:            u8,
	pub modulo_param:      u8,
	pub decay_param:       u8,
	pub is_listening_midi: u8,
	pub is_saving_preset:  u8,
	pub fns:               Box<[u8]>,
	pub name:              String,
}
impl DeviceConfig {
	pub fn get(&self, key: String) -> Option<u8> {
		let json = serde_json::to_value(self).unwrap();

		if let Value::Object(map) = json {
			if key != "name".to_string() 
			   || key != "fns".to_string() 
			{
				return Some(map.get(&key).unwrap().as_u64().unwrap() as u8);
			}
		}

		None
	}
	pub fn keys(&self) -> Vec<String> {
		let json = serde_json::to_value(self).unwrap();
		let mut keys = vec![];

		if let Value::Object(map) = json {
			for (key, _) in map {
				keys.push(key);
			}
		}

		keys
	}
}
#[derive(Debug)]
pub struct Midi {
	pub dev: DeviceInfo,
	pub cfg: DeviceConfig,
}

impl Midi {
	pub fn new(pm_ctx: &portmidi::PortMidi) -> Result<Self, Box<dyn std::error::Error>> {
		let devices = pm_ctx.devices()?;
		println!("[MIDI]: devices {:?}", devices);
		let mut config: HashMap<String, DeviceConfig> = 
			toml::from_str(&std::fs::read_to_string(*crate::CONF_FILE).unwrap()).unwrap_or_else(|e| {
				eprintln!("[MIDI]: Error reading config file: {e}");
				std::process::exit(1);
			});

		// TODO: rename config.toml to midi_config.toml
		println!("loaded midi config.toml {:#?}", config);

		let dev = devices.into_iter()
			.find(|d| {
				println!("device {:?}", d);
				d.direction() == portmidi::Direction::Input 
				&& config.keys().any(|n| n == d.name())
			});
			
		match dev {
			Some(d) => { 
				Ok(Self {
					cfg: unsafe { config.remove(d.name()).unwrap_unchecked() },
					dev: d
				}) 
			},
			None => {
				Err("couldn't init midi struct".into())
			},
		}

	}

	// TODO: setup a debugger?? :o
	pub fn handle_msg(&self, me: MidiEvent, ms: &mut crate::MutState) -> () {
		let channel   = me.message.data1;
		let intensity = me.message.data2;

		match self.cfg.name.as_str() {
			"XONE:K2 " | "XONE:K2" => self.handle_xonek2_msg(me, ms),
			"WINE ALSA Output #1"  => self.handle_ableton_msg(me, ms),
			"Pioneer DJ XDJ-RX2"   => self.handle_rx2_msg(me, ms),
			_ => {
				println!("[MIDI][INFO]: unknown controller name - not yet configured");
			},
		}

	}

	// private:
	fn handle_ableton_msg(&self, me: MidiEvent, ms: &mut crate::MutState) -> () {
		let channel   = me.message.data1;
		let intensity = me.message.data2;
		panic!("todo");
	}

	fn handle_rx2_msg(&self, me: MidiEvent, ms: &mut crate::MutState) -> () {
		let channel   = me.message.data1;
		let intensity = me.message.data2;
		panic!("todo");
	}

	fn handle_xonek2_msg(&self, me: MidiEvent, ms: &mut crate::MutState) -> () {
		let channel   = me.message.data1;
		let intensity = me.message.data2;

		let lerp_with_range = |range| crate::utils::lerp_float(intensity, 0.0, range, 0, 127);

		match channel {

			// latched boolean when condition matches
			c if c == self.cfg.backwards        && intensity == 127 => ms.is_backwards      = !ms.is_backwards,
			c if c == self.cfg.is_fft           && intensity == 127 => ms.save_state.is_fft = !ms.save_state.is_fft,

			c if c == self.cfg.is_listening_midi     && intensity == 127 => {
				println!("[MIDI]: is_listening_midi - true");
				ms.is_listening_midi = !ms.is_listening_midi;
			},
			c if c == self.cfg.is_saving_preset && intensity == 127 => {
				println!("[MIDI]: is_saving_preset - true");
			},
			c if c == self.cfg.is_saving_preset && intensity == 0   => {
				println!("[MIDI]: is_saving_preset - false");
			}

			// continuous control values
			c if c == self.cfg.intensity        => ms.save_state.current_intensity = lerp_with_range(ms.plugins[ms.save_state.active_func].intensity_range),
			c if c == self.cfg.decay_factor     => ms.save_state.decay_factor      = lerp_with_range(1.0),
			c if c == self.cfg.time_dialation   => ms.save_state.time_dialation    = lerp_with_range(ms.plugins[ms.save_state.active_func].time_dialation_range),
			c if c == self.cfg.lum_mod          => ms.save_state.lum_mod           = lerp_with_range(ms.plugins[ms.save_state.active_func].lum_mod),
			c if c == self.cfg.modulo_param     => ms.save_state.modulo_param      = lerp_with_range(368.0),
			c if c == self.cfg.decay_param      => ms.save_state.decay_param       = lerp_with_range(0.9999),

			// do nothing on zero for now...
			// because intensity is being used for division
			// better not allow division by zero or we'll panic!
			_ if intensity == 0 => (),

			// any other messages are probably unassigned config values 
			// or possibly the plugin function library collection indicies
			// latched on function to be activated from plugins
			// unfortunately this requires #[feature(if_let_guard)] 
			//
			// based on this PR it looks like it was temporarily there but they're trying to get it
			// into stable but the guy originally working on it is going on military service for at least a
			// year https://github.com/rust-lang/rust/pull/141295 
			//
			// c if let Some(i) = self.cfg.fns.iter().position(|&f| f == c) => ms.active_func = i,
			//
			// but it stopped working after I
			// updated rust. so i guess it was removed :( instead use nested match statement
			// and if the message on certain channels that are matching the function slice of
			// function locations in memory so i can poke at and reassign the visual patch
			//
			_ if intensity == 127 => { 
				// only setting the ms active_func if the cc was in the list of well_known_ccs or
				// the DeviceConfig control values
				if ms.well_known_ccs.iter().any(|cc| *cc == channel) {
					println!("[MIDI]: switching function midi channel used {:?}", channel);
					match self.cfg.fns.iter().position(|f| *f == channel) {
						Some(i) => { ms.save_state.active_func = i; },
						None => (), 
					}
				} else {
					// cc was not a device config cc - it's a user defined cc for a visual patch
					if !ms.is_listening_midi && ms.midi_config_fn_ccs.iter().any(|cc| *cc == channel) {

						// TODO: figure out how to override previously created configs
						// check if cc map already contains a key that matches the incoming message channel

						println!("[MIDI]: setting fn based on user cc mapping? {:?}", channel);

						// recall the entire save_state to the cc mapped state value structure(s)
						ms.save_state = ms.user_cc_map[&*format!("{}", channel)].clone()

					} else {
						// only if we're listening to create a new mapping
						// and save a new patch to a cc mapping during the nannou update()
						println!("[MIDI]: set save_state.cc to {:?}", channel);
						ms.save_state.cc = channel;
					}
				}
			},

			_ => {
				// not mapped by the config or the well-known cc list
				println!("[MIDI][UNASSIGNED?]: catch-all-match-arm got bogus amogus channel? {:?}", channel);
			},
		}
		// momentary switch
		ms.is_reset         = channel == self.cfg.reset            && intensity > 0;
		ms.is_saving_preset = channel == self.cfg.is_saving_preset && intensity > 0;
	}


}
