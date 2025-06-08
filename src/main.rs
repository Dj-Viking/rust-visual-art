#![allow(static_mut_refs)]
#![allow(unused_imports)]

use portmidi::PortMidi;

use nannou::prelude::*;
use nannou_audio;
use nannou_audio::Buffer;

use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;

mod midi;
mod loading;
mod audio_processor;

struct InputModel {
	producer: ringbuf::HeapProd<f32>,
}
struct OutputModel {
	consumer: ringbuf::HeapCons<f32>,
}

// these must be in the order of the file names
#[derive(Debug, Clone)]
#[repr(usize)]
enum ActiveFunc {
	Spiral = 0,
	V2,
	Waves,
	Audio,
	Solid
}

struct State {
	ms:              Arc<Mutex<MutState>>,
	consumer:        ringbuf::HeapCons<f32>,
	audio_processor: Arc<Mutex<audio_processor::AudioProcessor>>,
}

#[derive(Default)]
struct MutState {
	active_func:       usize,
	is_backwards:      bool,
	is_reset:          bool,
	is_fft:            bool,
	current_intensity: f32,
	time_dialation:    f32,
	decay_factor:      f32,
	lum_mod:           f32,
	modulo_param:      f32,
	decay_param:       f32,
	plugins:           Vec<loading::Plugin>,
}

static CONF_FILE:   LazyLock<&'static str> =
	LazyLock::new(|| std::env::var("CONF_FILE")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("config.toml"));

static PLUGIN_PATH: LazyLock<&'static str> =
	LazyLock::new(|| std::env::var("PLUGIN_PATH")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("target/libs"));

const SAMPLES: usize = 4096;
static mut PLUGS_COUNT: u8 = 0;

fn main() {

	// list midi devices in terminal
	if std::env::args().skip(1).any(|a| a == "list") {
		let pm_ctx = PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();
		devices.iter().for_each(|d| println!("{} {:?} {:?}", d.id(), d.name(), d.direction()));
		std::process::exit(0);
	}

	// get the amount of plugins as part of knowing when to reload them
	// when the watcher detects changes
	if let Ok(entries) = std::fs::read_dir(&*PLUGIN_PATH) {
		for entry in entries {
			if let Ok(_) = entry {
				unsafe { PLUGS_COUNT += 1 };
			}
		}
	}

	println!("plug count {}", unsafe { PLUGS_COUNT });

	let init = |a: &App| {
		let ms = Arc::new(Mutex::new(MutState {
			plugins: {
				let mut p = Vec::new();
				loading::Plugin::load_dir(*PLUGIN_PATH, &mut p);
				p
			},
			is_fft: false,
			..Default::default()
		}));

		// setup audio input
		let audio_host = nannou_audio::Host::new();

		let input_config = audio_host
			.default_input_device().unwrap()
			.default_input_config().unwrap();

		println!("default input {:#?}", input_config);

		let audio_processor = Arc::new(Mutex::new(audio_processor::AudioProcessor::new(
				input_config.sample_rate().0 as usize,
				60.0)));

		let ringbuffer = HeapRb::<f32>::new(SAMPLES * 2);

		let (mut prod, cons) = ringbuffer.split();

		for _ in 0..SAMPLES {
			prod.try_push(0.0).unwrap();
		}

		std::thread::spawn(move || {
			let in_model = InputModel { producer: prod };
			let in_stream = audio_host
				.new_input_stream(in_model)
				.capture(pass_in)
				.build()
				.unwrap();

			// TODO: flag for feedback configuration
			// let out_model = OutputModel { consumer: cons };
			// let out_stream = audio_host
			// 	.new_output_stream(out_model)
			// 	.render(pass_out)
			// 	.build()
			// 	.unwrap();

			// must be playing in a loop to keep the stream
			// open
			loop {
				in_stream.play().unwrap();
			}

			// only if you need to hear the audio played back through the same device used for
			// input
			// out_stream.play().unwrap();
		});

		let ms_ = ms.clone();

		// watch plugin file changes
		std::thread::spawn(move || {
			watch(&*PLUGIN_PATH, &ms_);
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
			ms,
			consumer: cons,
			audio_processor,
		}
	};

	nannou::app(init)
		.update(update)
		.run();
}

fn pass_in(model: &mut InputModel, buffer: &Buffer) -> () {

	buffer.frames().for_each(|f| {
		f.into_iter().for_each(|s| {
			let _ = model.producer.try_push(*s);
		});
	});

}

// only if you want to hear the audio output back into the
// device
#[allow(unused)]
fn pass_out(model: &mut OutputModel, buffer: &mut Buffer) -> () {

	buffer.frames_mut().for_each(|f| {
		f.iter_mut().for_each(|s| {
			*s = model.consumer.try_pop().unwrap_or(0.0)
		});
	});
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

	let set_active_func = |mut ms: MutexGuard<MutState>, n: ActiveFunc| match ms.plugins.len().cmp(&(n.clone() as usize)) {
		std::cmp::Ordering::Less => eprintln!("plugin {:?} not loaded", n),
		_ => ms.active_func = n as usize,
	};

	match key {
		Key::R => ms.is_reset = true,

		Key::Key1 => set_active_func(ms, ActiveFunc::Spiral),
		Key::Key2 => set_active_func(ms, ActiveFunc::V2),
		Key::Key3 => set_active_func(ms, ActiveFunc::Waves),
		Key::Key4 => set_active_func(ms, ActiveFunc::Audio),
		Key::Key5 => set_active_func(ms, ActiveFunc::Solid),

		Key::Up    if ms.current_intensity < 255.0 => ms.current_intensity += 1.0,
		Key::Down  if ms.current_intensity > 0.0   => ms.current_intensity -= 1.0,
		Key::Right if ms.time_dialation    < 255.0 => ms.time_dialation += 1.0,
		Key::Left  if ms.time_dialation    > 0.0   => ms.time_dialation -= 1.0,

		_ => (),
	}
}

fn update(_app: &App, state: &mut State,_update: Update) {
	
	let mut buffer = [0.0; 1024];

	state.consumer.pop_slice(&mut buffer);

	let mut ap = state.audio_processor.lock().unwrap();

	ap.add_samples(&buffer);

	// println!("============");
	// f32::memprint_block(&ap.buffer);
}

fn view(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let mut ms = s.ms.lock().unwrap();
	let ap = s.audio_processor.lock().unwrap();

	let mags = ap.get_magnitudes(ms.decay_param);
	
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

		if i % (ms.modulo_param + 1.0) as i32 == 0 {
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
		
		let t = unsafe { TIME } /
			(ms.plugins[ms.active_func].time_divisor + 100000.0 * (ms.time_dialation / 10.0))
			+ ms.current_intensity / 100.0;

		let hue = ms.plugins[ms.active_func].call(r.x(), r.y(), t);

		let lum = if ms.is_fft {
			lerp_float((mags[i as usize] + ms.lum_mod).ceil() as u8, 0.01, 0.6, 0, 100)
		} else { 0.5 };

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(hue, 1.0, lum);
	}

	draw.to_frame(app, &frame).unwrap();
}

fn watch(path: &str, ms_: &std::sync::Arc<Mutex<MutState>>) {
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
				event_count += 1;

				// wait for files to be fully removed
				// and recreated by the build script
				// kind of a weird hack because an event is fired for each File
				// in the plugin path but I need to wait for all the files
				// to be replaced
				if event_count == unsafe { PLUGS_COUNT * PLUGS_COUNT } {

					let mut ms = ms_.lock().unwrap();

					println!("\n=========\n watch event: {:?}", event.kind);

					println!("[INFO]: reloading plugins");
					ms.plugins.clear();
					loading::Plugin::load_dir(*PLUGIN_PATH, &mut ms.plugins);
					event_count = 0;
				}
			}
		},
		Err(error) => println!("Error: {:?}", error),
	} }
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
