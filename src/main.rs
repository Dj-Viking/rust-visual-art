use portmidi as pm;
use nannou::prelude::*;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Copy, Default)]
#[repr(u8)]
enum ActiveFunc {
	#[default]
	Spiral = 0,
	V2     = 1,
	Waves  = 2,
}

struct State {
	funcs: &'static [fn(f32, f32, f32) -> f32],
	ms:    Arc<Mutex<MutState>>,
}

#[derive(Default)]
struct MutState {
	is_backwards:      bool,
	is_reset:          bool,
	current_intensity: u8,
	time_dialiation:   u8,
	func:              ActiveFunc,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct DConfig {
	backwards:      u8,
	v2:             u8,
	waves:          u8,
	spiral:         u8,
	intensity:      u8,
	time_dialation: u8,
	reset:          u8,
}

const CONF_FILE: &str = "config.toml";

fn main() {
	if std::env::args().skip(1).any(|a| a == "list") {
		let pm_ctx = pm::PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();
		devices.iter().for_each(|d| println!("{} {:?} {:?}", d.id(), d.name(), d.direction()));
		std::process::exit(0);
	}

	let init = |a: &App| { 
		let ms = Arc::new(Mutex::new(MutState::default()));

		let pm_ctx = pm::PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();

		let (cfg, dev) = {
			let mut config: HashMap<String, DConfig> = 
				toml::from_str(&std::fs::read_to_string(CONF_FILE).unwrap()).unwrap_or_else(|e| {
					eprintln!("Error reading config file: {e}");
					std::process::exit(1);
				});

			let dev = devices.into_iter()
				.find(|d| d.direction() == pm::Direction::Input && config.keys().any(|n| n == d.name()))
				.unwrap_or_else(|| {
					eprintln!("No device defined in config found");
					std::process::exit(1);
				});

			(unsafe { config.remove(dev.name()).unwrap_unchecked() }, dev)
		};

		let ms_ = ms.clone();
		std::thread::spawn(move || {
			let mut in_port = pm_ctx.input_port(dev, 256).unwrap();

			loop {
				static mut BACKOFF: u8 = 0;
				// TODO: listen flag

				let Ok(Some(m)) = in_port.read() else {
					std::hint::spin_loop();
					std::thread::sleep(std::time::Duration::from_millis(unsafe { BACKOFF * 10 } as u64));
					unsafe { BACKOFF += 1; }
					unsafe { BACKOFF %= 10; }
					continue;
				};

				let channel   = m.message.data1;
				let intensity = m.message.data2;
				
				println!("chan {} - intensity {}", channel, intensity);

				let mut ms = ms_.lock().unwrap();

				if channel == cfg.intensity {
					ms.current_intensity = intensity;
				}

				if channel == cfg.time_dialation {
					ms.time_dialiation = intensity;
				}

				if channel == cfg.spiral && intensity > 0 {
					ms.func = ActiveFunc::Spiral;
				}

				if channel == cfg.v2 && intensity > 0 {
					ms.func = ActiveFunc::V2;
				}

				if channel == cfg.waves && intensity > 0 {
					ms.func = ActiveFunc::Waves;
				}

				ms.is_reset = channel == cfg.reset && intensity > 0;

				if channel == cfg.backwards && intensity > 0 {
					ms.is_backwards = !ms.is_backwards;
				}

				unsafe { BACKOFF = 0; }
			}
		});

		a.new_window()
			.view(update)
			.build().unwrap(); 

		State {
			ms,
			funcs: &[
				|y, x, t| y * x * t, // spiral
				|y, x, t| 32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05), // v2
				|y, x, t| x / y * t, // waves
			],
		}
	};

	nannou::app(init).run();
}

fn update(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);

	let f3 = |s: &State| {
		let ms = s.ms.lock().unwrap();
		match &ms.func {
			ActiveFunc::Spiral => s.funcs[ms.func as u8 as usize],
			ActiveFunc::V2     => s.funcs[ms.func as u8 as usize],
			ActiveFunc::Waves  => s.funcs[ms.func as u8 as usize],
		}
	};

	const TIME_DIVISOR: f32 = 1000000000.0;
	const TIME_DIVISOR2: f32 = 1000.0;
	static mut TIME: f32 = 0.0;

	let t = || unsafe {
		let mut ms = s.ms.lock().unwrap();

		match ms.is_backwards {
			true => TIME -= app.duration.since_prev_update.as_secs_f32(),
			_    => TIME += app.duration.since_prev_update.as_secs_f32(),
		}

		const THRESHOLD: f32 = 1000000000.0;
		if TIME >= THRESHOLD || TIME <= -THRESHOLD {
			ms.is_backwards = !ms.is_backwards;
		}

		if ms.is_reset { TIME = 0.0; } 
		
		TIME /
			(if ms.func == ActiveFunc::Waves { TIME_DIVISOR2 } else { TIME_DIVISOR } 
			+ 100000.0 * ms.time_dialiation as f32)
			+ ms.current_intensity as f32 / 100.0
	};

	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter()) {
		let hue = f3(s)(r.y(), r.x(), t());

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(hue, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}
