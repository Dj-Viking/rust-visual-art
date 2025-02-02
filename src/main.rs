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
	Solid  = 3,
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
	time_dialation:    u8,
	func:              ActiveFunc,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct DConfig {
	backwards:      u8,
	v2:             u8,
	waves:          u8,
	solid:          u8,
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

		if !std::env::args().skip(1).any(|a| a == "keys") {

			println!("[INFO]: now running in keyboard mode using midi controller");

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
						ms.time_dialation = intensity;
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

					if channel == cfg.solid && intensity > 0 {
						ms.func = ActiveFunc::Solid;
					}

					ms.is_reset = channel == cfg.reset && intensity > 0;

					if channel == cfg.backwards && intensity > 0 {
						ms.is_backwards = !ms.is_backwards;
					}

					unsafe { BACKOFF = 0; }
				}
			});
		}

		a.new_window()
			.view(update)
			.key_pressed(key_pressed)
			.key_released(key_released)
			.build().unwrap(); 

		State {
			ms,
			funcs: &[
				|y, x, t| y * x * t, // spiral
				|y, x, t| 32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05), // v2
				|y, x, t| x / y * t, // waves
				|y, x, t| (x % 2.0 + 1000.0) / (y % 2.0 + 1000.0) * (t), // solid
			],
		}
	};

	nannou::app(init).run();
}
fn key_released(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();
	match key {
		Key::R => { ms.is_reset = false; },
		_ => ()
	}
}

fn key_pressed(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();
	match key {
		Key::R => { ms.is_reset = true; },

		Key::S => ms.func = ActiveFunc::Spiral,
		Key::W => ms.func = ActiveFunc::Waves,
		Key::O => ms.func = ActiveFunc::Solid,
		Key::V => ms.func = ActiveFunc::V2,

		Key::Up    => { if ms.current_intensity < 255 { ms.current_intensity += 1; } },
		Key::Down  => { if ms.current_intensity > 0   { ms.current_intensity -= 1; } },
		Key::Right => { if ms.time_dialation    < 255 { ms.time_dialation += 1; } },
		Key::Left  => { if ms.time_dialation    > 0   { ms.time_dialation -= 1; } },

		_ => (),
	}
}


fn update(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let mut ms = s.ms.lock().unwrap();

	static mut TIME: f32 = 0.0;

	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter()) {
		let time_divisor = match ms.func {
			ActiveFunc::Waves => 1000.0,
			ActiveFunc::Solid => 1000.0,
			_                 => 1000000000.0,
		};

		match ms.is_backwards {
			true => unsafe { TIME -= app.duration.since_prev_update.as_secs_f32() },
			_    => unsafe { TIME += app.duration.since_prev_update.as_secs_f32() },
		}

		const THRESHOLD: f32 = 1000000000.0;
		if unsafe { TIME >= THRESHOLD || TIME <= -THRESHOLD } {
			ms.is_backwards = !ms.is_backwards;
		}

		if ms.is_reset { unsafe { TIME = 0.0; } } 
		
		let t = unsafe { TIME } /
			(time_divisor + 100000.0 * (ms.time_dialation as f32 / 10.0))
			+ ms.current_intensity as f32 / 100.0;

		let hue = s.funcs[ms.func as u8 as usize](r.y(), r.x(), t);

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(hue, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}
