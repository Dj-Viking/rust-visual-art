#![allow(static_mut_refs)]

use portmidi::PortMidi;

use nannou::prelude::*;
use nannou_audio::Buffer;

use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;

mod args;

mod midi;
mod loading;
mod audio_processor;
mod utils;
mod save_state;
mod mutstate;

use save_state::SaveState;
use mutstate::MutState;

struct InputModel {
	producer: ringbuf::HeapProd<f32>,
}

// struct OutputModel {
// 	consumer: ringbuf::HeapCons<f32>,
// }

struct State {
	ms:              Arc<Mutex<MutState>>,
	consumer:        ringbuf::HeapCons<f32>,
	audio_processor: Arc<Mutex<audio_processor::AudioProcessor>>,
}


static PRESETS_DIR: LazyLock<String> =
	LazyLock::new(|| std::env::var("PRESETS_DIR")
		.unwrap_or(String::from("presets")));

static CONF_FILE:      LazyLock<&'static str> =
	LazyLock::new(|| std::env::var("CONF_FILE_PATH")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("config.toml"));

static PLUGIN_PATH: LazyLock<String> =
	LazyLock::new(|| std::env::var("PLUGIN_PATH")
		.unwrap_or(String::from("target/libs")));

const SAMPLES: usize = 4096;

fn main() {
	let init = |a: &App| {
		let pm_ctx = PortMidi::new().expect("could not get midi ctx");

		let (controller_name, midi) = match midi::Midi::new(&pm_ctx) {
			Ok(midi) => (midi.cfg.name.clone(), Some(midi)),
			Err(e)   => {
				println!("{e}");
				println!("[MAIN][INFO]: keyboard mode only running"); 
				(String::from("default"), None)
			},
		};

		let ms = Arc::new(Mutex::new(MutState {
			preset_map: SaveState::from_dir(&*PRESETS_DIR),
			save_state: SaveState::from_file(Path::new(&*PRESETS_DIR).join("default.toml"))
				.unwrap_or_default(),
			controller_name,
			plugins: {
				let mut p = Vec::new();
				loading::Plugin::load_dir(&*PLUGIN_PATH, &mut p);
				p
			},
			..Default::default()
		}));

		// initialize midi stuff
		if let Some(midi) = midi {
			let ms_ = ms.clone();
			std::thread::spawn(move || {
				let mut in_port = pm_ctx.input_port(midi.dev.clone(), 256).unwrap();
				let mut backoff: u32 = 0;
				loop {
					let Ok(Some(m)) = in_port.read() else {
						std::hint::spin_loop();
						std::thread::sleep(std::time::Duration::from_millis((backoff * 10) as u64));
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

		// setup audio input
		let audio_host = nannou_audio::Host::new();

		let input_config = audio_host
			.default_input_device().unwrap()
			.default_input_config().unwrap();

		println!("[MAIN]: default input {:#?}", input_config);

		let audio_processor = Arc::new(Mutex::new(audio_processor::AudioProcessor::new(
				input_config.sample_rate().0 as usize,
				60.0)));

		let ringbuffer = HeapRb::<f32>::new(SAMPLES * 2);

		let (mut prod, cons) = ringbuffer.split();

		(0..SAMPLES).for_each(|_| prod.try_push(0.0).unwrap());

		std::thread::spawn(move || {
			let in_model = InputModel { producer: prod };
			let in_stream = audio_host
				.new_input_stream(in_model)
				.capture(pass_in)
				.build()
				.unwrap();

			loop {
				in_stream.play().unwrap();
			}
		});

		if args::ARGS.hmr_enable {
			let plugs_count = std::fs::read_dir(&*PLUGIN_PATH).map_or(0, 
				|e| e.filter(|e| e.as_ref().is_ok_and(|p| p.path().is_file())).count())
				.try_into().unwrap_or(0u8); // FIXME: this is kinda bad since it limits us to 63 presets max

			let ms_ = ms.clone();
			std::thread::spawn(move || {
				utils::watch(plugs_count, &PLUGIN_PATH, &ms_);
			});
		}

		a.new_window()
			.view(view)
			.key_pressed(key_pressed)
			.key_released(key_released)
			.build().unwrap();

		State {
			ms,
			consumer: cons,
			audio_processor,
		}
	};

	nannou::app(init)
		.update(update)
		.run();
}

fn pass_in(model: &mut InputModel, buffer: &Buffer) {
	buffer.frames().for_each(|f| 
		f.iter().for_each(|s| { 
			let _ = model.producer.try_push(*s); }));
}

fn key_released(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();
	match key {
		Key::R => ms.is_reset         = false,
		Key::P => { println!("[MAIN][UTILS]: is_saving_preset false");
			      ms.is_saving_preset = false;
		},
		_ => ()
	}
}

fn key_pressed(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();

	match key {
		Key::A => ms.save_state.is_fft      = !ms.save_state.is_fft,
		Key::R => ms.is_reset    = true,
		Key::P => {
			println!("[Main]: is_saving_preset true");
			ms.is_saving_preset  = true;
		},
		Key::L => {
			println!("[MAIN][KEYS]: is_listening_keys true");
			ms.is_listening_keys = true;
		}

		Key::Key1 => ms.set_active_func(0),
		Key::Key2 => ms.set_active_func(1),
		Key::Key3 => ms.set_active_func(2),
		Key::Key4 => ms.set_active_func(3),
		Key::Key5 => ms.set_active_func(4),
		Key::Key6 => ms.set_active_func(5),
		Key::Key7 => ms.set_active_func(6),
		Key::Key8 => ms.set_active_func(7),
		Key::Key9 => ms.set_active_func(8),
		Key::Key0 => ms.set_active_func(9),

		Key::LBracket => {
			let (id, overflow) = ms.save_state.active_func.overflowing_sub(1);
			let id = if overflow { ms.plugins.len() - 1 } else { id };
			ms.set_active_func(id);
		},
		Key::RBracket => {
			ms.save_state.active_func += 1;
			if ms.save_state.active_func >= ms.plugins.len() {
				ms.save_state.active_func = 0;
			}
			println!("[MAIN]: active func {:?}", ms.save_state.active_func);
		},

		Key::Up    if ms.save_state.current_intensity < 255.0 => ms.save_state.current_intensity += 0.1,
		Key::Down  if ms.save_state.current_intensity > 0.0   => ms.save_state.current_intensity -= 0.1,
		Key::Right if ms.save_state.time_dialation    < 255.0 => ms.save_state.time_dialation    += 0.1,
		Key::Left  if ms.save_state.time_dialation    > 0.0   => ms.save_state.time_dialation    -= 0.1,

		_ => (),
	}
}

fn update(_app: &App, state: &mut State,_update: Update) {
	let mut ms = state.ms.lock().unwrap();

	if ms.save_state.is_fft {
		let mut buffer = [0.0; 1024];

		state.consumer.pop_slice(&mut buffer);

		let mut ap = state.audio_processor.lock().unwrap();

		ap.add_samples(&buffer);
	}

	ms.save_preset().unwrap_or_else(|e| {
		eprintln!("[MAIN]: Error saving preset: {e}");
	});
}


fn view(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let mut ms = s.ms.lock().unwrap();
	let ap = s.audio_processor.lock().unwrap();

	let mags = ap.get_magnitudes(ms.save_state.decay_param);
	
	static mut TIME: f32 = 0.0;

	const UPPER_TIME_LIMIT: f32 = 524288.0;
	const LOWER_TIME_LIMIT: f32 = -524288.0;
	if unsafe { TIME >= UPPER_TIME_LIMIT || TIME <= LOWER_TIME_LIMIT } {
		ms.is_backwards = !ms.is_backwards;
	}
	
	let mut i: i32 = -1;
	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
	{
		i += 1;

		// TODO: figure out how to get mags.len() to 4096!
		// if i == mags.len() {

		if i % (ms.save_state.modulo_param + 1.0) as i32 == 0 {
			i = 0;
		}

		match ms.is_backwards {
			true => unsafe { TIME -= app.duration.since_prev_update.as_secs_f32() },
			_    => unsafe { TIME += app.duration.since_prev_update.as_secs_f32() },
		}

		const THRESHOLD: f32 = 1000000000.0;
		if unsafe { TIME >= THRESHOLD || TIME <= -THRESHOLD } {
			ms.is_backwards = !ms.is_backwards;
		}

		if ms.is_reset { unsafe { TIME = 0.0; } }


		const TIME_OFFSET: f32 = 100000.0;

		let mut hue: f32 = 0.0;
		if !ms.plugins.is_empty() {
			let t: f32 = unsafe { TIME } / (
					ms.plugins[ms.save_state.active_func].time_divisor
					+ TIME_OFFSET
					* (ms.save_state.time_dialation / 10.0)
				)
				+ ms.save_state.current_intensity / 100.0;

			hue = ms.plugins[ms.save_state.active_func].call(r.x(), r.y(), t);
		}

		let lum = if ms.save_state.is_fft {
			utils::lerp_float((mags[i as usize] + ms.save_state.lum_mod).ceil() as u8, 0.01, 0.6, 0, 100)
		} else { 0.5 };

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(hue, 1.0, lum);
	}

	draw.to_frame(app, &frame).unwrap();
}
