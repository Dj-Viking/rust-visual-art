use portmidi::{DeviceInfo, MidiEvent};
use std::collections::HashMap;

use crate::MutState;

#[derive(serde::Deserialize, serde::Serialize)]
struct DConfig {
	backwards:      u8,
	fns:            Box<[u8]>,
	intensity:      u8,
	time_dialation: u8,
	decay_factor:   u8,
	lum_mod:        u8,
	reset:          u8,
	is_fft:         u8,
	modulo_param:   u8,
	decay_param:    u8,
}

pub struct Midi {
	pub dev: DeviceInfo,
	cfg: DConfig,
}

impl Midi {
	pub fn new(pm_ctx: &portmidi::PortMidi) -> Result<Self, Box<dyn std::error::Error>> {
		let devices = pm_ctx.devices()?;
		let mut config: HashMap<String, DConfig> = 
			toml::from_str(&std::fs::read_to_string(*crate::CONF_FILE).unwrap()).unwrap_or_else(|e| {
				eprintln!("Error reading config file: {e}");
				std::process::exit(1);
			});

		let dev = devices.into_iter()
			.find(|d| 
				d.direction() == portmidi::Direction::Input 
				&& config.keys().any(|n| n == d.name()))
			.unwrap_or_else(|| {
				eprintln!("No device defined in config found - \ndid you plug in a MIDI controller yet?\nOr have you configured your controller for the config.toml file yet?");
				std::process::exit(1);
			});

		Ok(Self {
			cfg: unsafe { config.remove(dev.name()).unwrap_unchecked() },
			dev,
		})
	}

	pub fn handle_msg(&self, me: MidiEvent, ms: &mut MutState) {
		let channel   = me.message.data1;
		let intensity = me.message.data2;

		let lerp_with_range = |range| crate::lerp_float(intensity, 0.0, range, 0, 127);

		match channel {

			// latched boolean when condition matches
			c if c == self.cfg.backwards && intensity == 127 => ms.is_backwards = !ms.is_backwards,
			c if c == self.cfg.is_fft    && intensity == 127 => ms.is_fft       = !ms.is_fft,

			// continuous control values
			c if c == self.cfg.intensity      => ms.current_intensity = lerp_with_range(ms.plugins[ms.active_func].intensity_range),
			c if c == self.cfg.decay_factor   => ms.decay_factor      = lerp_with_range(1.0),
			c if c == self.cfg.time_dialation => ms.time_dialation    = lerp_with_range(ms.plugins[ms.active_func].time_dialation_range),
			c if c == self.cfg.lum_mod        => ms.lum_mod           = lerp_with_range(ms.plugins[ms.active_func].lum_mod),
			c if c == self.cfg.modulo_param   => ms.modulo_param      = lerp_with_range(368.0),
			c if c == self.cfg.decay_param    => ms.decay_param       = lerp_with_range(0.9999),

			_ if intensity == 0 => (),

			// any other messages are probably unassigned config values 
			// or possibly the plugin function library collection indicies
			// latched on function to be activated from plugins
			// unfortunately this requires #[feature(if_let_guard)] 
			//
			// c if let Some(i) = self.cfg.fns.iter().position(|&f| f == c) => ms.active_func = i,
			//
			// but it stopped working after I
			// updated rust. so i guess it was removed :( instead use nested match statement
			// and if the message on certain channels that are 
			_ if intensity == 127 => match self.cfg.fns.iter().position(|f| *f == channel) {
				Some(i) => { ms.active_func = i; },
				None => (),
			},
			_ => () 
		}



		// momentary switch
		ms.is_reset = channel == self.cfg.reset && intensity > 0;

	}
}

