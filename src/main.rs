#![allow(static_mut_refs)]
#![feature(if_let_guard)]

use portmidi::PortMidi;
use nannou::prelude::*;

use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

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

static CONF_FILE:   LazyLock<&'static str> = 
	LazyLock::new(|| std::env::var("CONF_FILE")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("config.toml"));

static PLUGIN_PATH: LazyLock<&'static str> = 
	LazyLock::new(|| std::env::var("PLUGIN_PATH")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("target/libs"));

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
			plugins: { 
				let mut p = Vec::new(); 
				loading::Plugin::load_dir(*PLUGIN_PATH, &mut p); 
				p
			},
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

		State { ms, sample_rate }
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

	let set_active_func = |mut ms: MutexGuard<MutState>, n| match ms.plugins.len().cmp(&n) {
		std::cmp::Ordering::Less => eprintln!("plugin {n} not loaded"),
		_ => ms.active_func = n,
	};

	match key {
		Key::R => ms.is_reset = true,
		Key::L => {
			println!("[INFO]: reloading plugins");
			ms.plugins.clear();
			loading::Plugin::load_dir(*PLUGIN_PATH, &mut ms.plugins);
		},

		Key::Key1 => set_active_func(ms, 0),
		Key::Key2 => set_active_func(ms, 1),
		Key::Key3 => set_active_func(ms, 2),
		Key::Key4 => set_active_func(ms, 3),
		Key::Key5 => set_active_func(ms, 4),
		Key::Key6 => set_active_func(ms, 5),
		Key::Key7 => set_active_func(ms, 6),
		Key::Key8 => set_active_func(ms, 7),
		Key::Key9 => set_active_func(ms, 8),
		Key::Key0 => set_active_func(ms, 9),

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


	let fft = unsafe { 
		Vec::from_raw_parts(
			fft_data.data().as_ptr() as *mut (f32, f32),
			fft_data.data().len(),
			fft_data.data().len()) 
	};
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
			(ms.plugins[ms.active_func].time_divisor + 100000.0 * (ms.time_dialation / 10.0))
			+ ms.current_intensity / 100.0;

		let val = ms.plugins[ms.active_func].call(r.x(), r.y(),  t, &fft);

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(val, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}

