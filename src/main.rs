#![allow(static_mut_refs)]
#![feature(if_let_guard)]

use portmidi::PortMidi;
use nannou::prelude::*;

use std::sync::{Arc, Mutex};

mod audio;
mod midi;
mod loading;

struct State {
	ms:          Arc<Mutex<MutState>>,
	sample_rate: u32,
}

#[derive(Default)]
struct MutState {
	is_backwards:      bool,
	is_reset:          bool,
	current_intensity: f32,
	time_dialation:    f32,
	decay_factor:      f32,
	plugins:           Vec<loading::Plugin>,
	active_func:       usize,
}

const CONF_FILE:   &str = "config.toml";
const PLUGIN_PATH: &str = "target/libs";

fn main() {
	// list midi devices in terminal
	if std::env::args().skip(1).any(|a| a == "list") {
		let pm_ctx = PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();
		devices.iter().for_each(|d| println!("{} {:?} {:?}", d.id(), d.name(), d.direction()));
		std::process::exit(0);
	}

	let init = |a: &App| { 
		let ms = Arc::new(Mutex::new(MutState {
			plugins: loading::Plugin::load_dir(PLUGIN_PATH),
			..Default::default()
		}));

		let mut audio = audio::Audio::init().unwrap();
		let sample_rate = audio.sample_rate();

		// audio stream thread
		std::thread::spawn(move || {
			loop {
				std::thread::sleep(std::time::Duration::from_millis(1));
				audio.read_stream().unwrap()
			}
		});

		let ms_ = ms.clone();
		if !std::env::args().skip(1).any(|a| a == "keys") {
			let pm_ctx = PortMidi::new().unwrap();
			let midi = midi::Midi::new(&pm_ctx).unwrap();

			std::thread::spawn(move || {
				let mut in_port = pm_ctx.input_port(midi.dev.clone(), 256).unwrap();

				let mut backoff = 0;
				loop {
					// TODO: listen flag

					let Ok(Some(m)) = in_port.read() else {
						std::hint::spin_loop();

						std::thread::sleep(
							std::time::Duration::from_millis(
								(backoff * 10) as u64));

						backoff += 1;
						backoff %= 10;
						continue;
					};

					let mut ms = ms_.lock().unwrap();
					midi.handle_msg(m, &mut ms);
					backoff = 0;
				}
			});
		}

		a.new_window()
			.view(view)
			.key_pressed(key_pressed)
			.key_released(key_released)
			.build().unwrap(); 

		State {
			ms, sample_rate,
			// funcs: &[
			// 	|y, x, t, _, _| 32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05), // v2func
			// 	|y, x, t, _, _| x / y * t, // wavesfunc
			// 	|y, x, t, _, _| (x % 2.0 + 1000.0) / (y % 2.0 + 1000.0) * (t), // solidfunc
			// 	|y, x, t, fft_data, _| { // audiofunc
			//
			// 		// magnitudes are huge coming from fft_data
			// 		// lets make it a usable number for our situation
			// 		// can noise clamp be controllable?
			// 		const NOISE_CLAMP: f32 = 10.0;
			// 		const FREQ_AVERAGE: f32 = 500.0;
			// 		const MAG_DIVISOR: f32 = 1000000.0;
			//
			// 		let mut magthing = fft_data.iter()
			// 			.map(|&(f, m)| (f, m / MAG_DIVISOR))
			// 			.find(|&(f, _)| f >= FREQ_AVERAGE)
			// 			.and_then(|(_, m)| (m > NOISE_CLAMP).then_some(m))
			// 			.unwrap_or(0.0);
			//
			// 		// can't get around the noise - not sure what to do with this yet
			// 		if magthing < 101.0 { magthing = 0.0 }
			// 		(y - magthing) * (x + magthing) * t / 100.0
			// 	}
			// ],
		}
	};

	nannou::app(init).run();
}

fn key_released(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();
	match key {
		Key::R => ms.is_reset = false,
		_ => ()
	}
}

fn key_pressed(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();
	match key {
		Key::R => ms.is_reset = true,

		Key::Key1 => ms.active_func = 0,
		Key::Key2 => ms.active_func = 1,
		Key::Key3 => ms.active_func = 2,
		Key::Key4 => ms.active_func = 3,
		Key::Key5 => ms.active_func = 4,
		Key::Key6 => ms.active_func = 5,
		Key::Key7 => ms.active_func = 6,
		Key::Key8 => ms.active_func = 7,
		Key::Key9 => ms.active_func = 8,
		Key::Key0 => ms.active_func = 9,

		Key::Up    if ms.current_intensity < 255.0 => ms.current_intensity += 1.0,
		Key::Down  if ms.current_intensity > 0.0   => ms.current_intensity -= 1.0,
		Key::Right if ms.time_dialation    < 255.0 => ms.time_dialation += 1.0,
		Key::Left  if ms.time_dialation    > 0.0   => ms.time_dialation -= 1.0,

		_ => (),
	}
}

fn view(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let mut ms = s.ms.lock().unwrap();

	let fft_data = spectrum_analyzer::samples_fft_to_spectrum(
		&spectrum_analyzer::windows::hann_window(unsafe { &audio::SAMPLEBUF }),
		s.sample_rate,
		spectrum_analyzer::FrequencyLimit::Range(50.0, 12000.0),
		Some(&spectrum_analyzer::scaling::divide_by_N_sqrt)
	).unwrap();


	let fft = unsafe { Vec::from_raw_parts(
		fft_data.data().as_ptr() as *mut (f32, f32),
		fft_data.data().len(),
		fft_data.data().len()) };
	std::mem::forget(fft_data);


	// a pretty good decay factor
	// controlled by midi but here for reference
	// gives a slow smeary like feeling
	const FACTOR: f32 = 0.9999;
	static mut PREV_FFT: Vec<(f32, f32)> = Vec::new();

	fft.iter().map(|(_, m)| m)
		.zip(unsafe { PREV_FFT.iter_mut().map(|(_, m)| m) })
		.for_each(|(c, p)| 
			if *c > *p { *p = *c; } 
			else { *p *= FACTOR; });


	static mut TIME: f32 = 0.0;

	// const UPPER_TIME_LIMIT: f32 = 524288.0;
	// const LOWER_TIME_LIMIT: f32 = -524288.0;
	// if unsafe { TIME >= UPPER_TIME_LIMIT || TIME <= LOWER_TIME_LIMIT } {
	// 	ms.is_backwards = !ms.is_backwards;
	// }

	// FIXME
	// let time_divisor = match ms.func {
	// 	ActiveFunc::Waves | ActiveFunc::Solid => 1000.0,
	// 	_ => ,
	// };

	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter()) {

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
			(1000100000.0 * (ms.time_dialation / 10.0))
			+ ms.current_intensity / 100.0;

		let val = ms.plugins[ms.active_func].call(r.y(), r.x(), t, &fft);

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(val, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}
