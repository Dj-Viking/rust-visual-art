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
	reset:          u8,
}

pub struct Midi {
	pub dev: DeviceInfo,
	cfg: DConfig,
}

impl Midi {
	pub fn new(pm_ctx: &portmidi::PortMidi) -> Result<Self, Box<dyn std::error::Error>> {
		let devices = pm_ctx.devices()?;
		let mut config: HashMap<String, DConfig> = 
			toml::from_str(&std::fs::read_to_string(crate::CONF_FILE).unwrap()).unwrap_or_else(|e| {
				eprintln!("Error reading config file: {e}");
				std::process::exit(1);
			});


		let dev = devices.into_iter()
			.find(|d| 
				d.direction() == portmidi::Direction::Input 
				&& config.keys().any(|n| n == d.name()))
			.unwrap_or_else(|| {
				eprintln!("No device defined in config found");
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

		// FIXME
		// let max_intensity_range = match ms.func {
		// 	ActiveFunc::Solid | ActiveFunc::Waves => 100.0,
		// 	_ => 0.01,
		// };

		match channel {
			c if c == self.cfg.backwards => ms.is_backwards = !ms.is_backwards,

			c if c == self.cfg.intensity      => ms.current_intensity = lerp_float(intensity, 0.0, 100.0, 0, 127),
			c if c == self.cfg.decay_factor   => ms.decay_factor      = lerp_float(intensity, 0.0, 1.0, 0, 127),
			c if c == self.cfg.time_dialation => ms.time_dialation    = lerp_float(intensity, 0.0, 100.0, 0, 127),
			_ if intensity == 0 => (),

			c if let Some(i) = self.cfg.fns.iter().position(|&f| f == c) => ms.active_func = i,
			_ => (),
		}

		ms.is_reset = channel == self.cfg.reset && intensity > 0;
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
