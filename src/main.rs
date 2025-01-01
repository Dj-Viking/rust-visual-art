use portmidi as pm;
use nannou::prelude::*;
use std::thread;

use std::sync::{LazyLock, Arc, Mutex};

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
struct Config {
	device_name: Box<str>,
	backwards_c:      u8,
	v2_c:             u8,
	waves_c:          u8,
	spiral_c:         u8,
	intensity_c:      u8,
	time_dialation_c: u8,
	reset_c:          u8,
}

const CONF_FILE: &str = "config.toml";
static CONFIG: LazyLock<Config> = LazyLock::new(||
	toml::from_str(&std::fs::read_to_string(CONF_FILE).unwrap()).unwrap_or_else(|e| {
		eprintln!("Error reading config file: {e}");
		std::process::exit(1);
	}));

fn main() {
	
	// get user input to choose available controllers that may be configured by the user already
	// and use those mappings
	if std::env::args().skip(1).any(|a| a == "list") {
		let pm_ctx = pm::PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();
		devices.iter().for_each(|d| println!("{} {:?} {:?}", d.id(), d.name(), d.direction()));
		std::process::exit(0);
	}

	let _ = &*CONFIG; // make sure config is initialized beforehand

	let init = |a: &App| { 
		let ms = Arc::new(Mutex::new(MutState::default()));

		let pm_ctx = pm::PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();

		let dev = devices.into_iter().find(|d| 
			d.name() == &*CONFIG.device_name && d.direction() == pm::Direction::Input)
			.unwrap_or_else(|| {
				eprintln!("No device found with name: {}", &*CONFIG.device_name);
				std::process::exit(1);
			});

		let ms_ = ms.clone();
		thread::spawn(move || {
			let in_port = pm_ctx.input_port(dev, 1024).unwrap();

			while in_port.poll().is_ok() {
				static mut BACKOFF: u8 = 0;

				// handle midi message
				if let Ok(Some(m)) = in_port.read_n(1024) {
					println!("{:?}", m[0]);
					let channel = m[0].message.data1;
					let intensity = m[0].message.data2;

					let mut ms = ms_.lock().unwrap();

					// continuous value 0-127
					if channel == CONFIG.intensity_c {
						ms.current_intensity = intensity;
					}

					// continuous value 0-127
					if channel == CONFIG.time_dialation_c {
						ms.time_dialiation = intensity;
					}

					// latch behavior
					if channel == CONFIG.spiral_c && intensity > 0 {
						ms.func = ActiveFunc::Spiral;
					}

					// latch behavior
					if channel == CONFIG.v2_c && intensity > 0 {
						ms.func = ActiveFunc::V2;
					}

					// latch behavior
					if channel == CONFIG.waves_c && intensity > 0 {
						ms.func = ActiveFunc::Waves;
					}

					// momentary switch behavior
					if channel == CONFIG.reset_c && intensity > 0 {
						ms.is_reset = true;
					} else { 
						ms.is_reset = false; 
					}

					// toggle behavior
					if channel == CONFIG.backwards_c && intensity > 0 {
						ms.is_backwards = !ms.is_backwards;
					}

					unsafe { BACKOFF = 0; }
					continue;
				}

				std::hint::spin_loop();
				std::thread::sleep(std::time::Duration::from_millis(unsafe { BACKOFF * 10 } as u64));
				unsafe { BACKOFF += 1; }
				unsafe { BACKOFF %= 10; }
			}
		});

		a.new_window()
			.view(update)
			.build().unwrap(); 

		State {
			ms,
			funcs: &[
				|y, x, t| y * x * t, // spiral
				|y, x, t| 32.0 / (t / x) + y / // v2
					(x / y - 1.0 / t) +
					t * (y * 0.05),
				|y, x, t| x / y * t
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
			ActiveFunc::Waves     => s.funcs[ms.func as u8 as usize],
		}
	};

	const TIME_DIVISOR: f32 = 1000000000.0;
	const TIME_DIVISOR2: f32 = 1000.0;
	static mut TIME: f32 = 0.0;

	let t = || unsafe {
		let mut ms = s.ms.lock().unwrap();
		if ms.is_backwards { 
			TIME -= 
				app.duration.since_prev_update.as_secs_f32()
		} 
		else { 
			TIME += 
				app.duration.since_prev_update.as_secs_f32()
		}

		const THRESHOLD: f32 = 1000000000.0;
		if TIME >= THRESHOLD || TIME <= -THRESHOLD {
			ms.is_backwards = !ms.is_backwards;
		}

		if ms.is_reset { TIME = 0.0; } 
		
		if &ms.func == &ActiveFunc::Waves {
			TIME / 
				(TIME_DIVISOR2 + (100000.0 * ms.time_dialiation as f32)) 
				+ ms.current_intensity as f32 / 100.0
				
		} else {
			TIME / 
				(TIME_DIVISOR + (100000.0 * ms.time_dialiation as f32)) 
				+ ms.current_intensity as f32 / 100.0
		}
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
