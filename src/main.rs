#![allow(static_mut_refs)]
#![feature(if_let_guard)]

use portmidi::PortMidi;

use nannou::prelude::*;
use nannou_audio as audio;
use nannou_audio::Buffer;

use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;

use rustfft::{Fft, FftPlanner};
use rustfft::num_complex::Complex;

use std::cmp::Ordering;

mod midi;
mod loading;

struct InputModel {
	producer: ringbuf::HeapProd<f32>,
}
struct OutputModel {
	consumer: ringbuf::HeapCons<f32>,
}

struct AudioProcessor {
	pub buffer: Vec<f32>,
	pub buffer_size: usize,
	fft: Arc<dyn Fft<f32>>,
}
impl AudioProcessor {
	fn new(sample_rate: usize, frame_rate: f32) -> Self {
		let buffer_size = (sample_rate as f32 / frame_rate).ceil() as usize;
		let mut planner: FftPlanner<f32> = FftPlanner::new();
		let fft = planner.plan_fft_forward(buffer_size);

		Self {
			buffer: vec![0.0; buffer_size],
			buffer_size,
			fft,
		}
	}

	fn add_samples(&mut self, samples: &[f32]) {
		self.buffer.extend_from_slice(samples);

		// deal with possible race condition of the sketch
		// update happening and requesting data before buffer is full.
		// fft buffer may be too small before processing it
		match self.buffer.len().cmp(&self.buffer_size) {
			Ordering::Greater => {
				self.buffer.drain(0..(self.buffer.len() - self.buffer_size));
			},
			Ordering::Less => {
				while self.buffer.len() < self.buffer_size {
					self.buffer.push(0.0);
				}
			},
			_ => {},
		}
	}

	fn get_magnitudes(&self) -> Vec<f32> {

		// TODO: add smoothing factor to the buffer samples
		// before creating complex input

		let mut complex_input: Vec<Complex<f32>> = 
			self.buffer.iter()
			.map(|&x| Complex::new(x, 0.0)).collect();

		self.fft.process(&mut complex_input);

		let mags = complex_input
			.iter().map(|c| {
				let mag = c.norm() / complex_input.len() as f32;
				20.0 * (mag.max(1e-8)).log10()
			})
			.collect::<Vec<f32>>();

		mags
	}
}

struct State {
	ms:              Arc<Mutex<MutState>>,
	consumer:        ringbuf::HeapCons<f32>,
	audio_processor: Arc<Mutex<AudioProcessor>>,
}

#[derive(Default)]
struct MutState {
	is_backwards:      bool,
	is_reset:          bool,
	is_fft:            bool,
	current_intensity: f32,
	time_dialation:    f32,
	decay_factor:      f32,
	sat_mod:           f32,
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


		let audio_host = audio::Host::new();

		let input_config = audio_host
			.default_input_device().unwrap()
			.default_input_config().unwrap();
		println!("default input {:#?}", 
			input_config);

		let audio_processor = Arc::new(Mutex::new(
			AudioProcessor::new(
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

			// let out_model = OutputModel { consumer: cons_, ap: ap__ };
			// let out_stream = audio_host
			// 	.new_output_stream(out_model)
			// 	.render(pass_out)
			// 	.build()
			// 	.unwrap();

			loop {
				in_stream.play().unwrap();
				// out_stream.play().unwrap();
			}
		});

		let ms_ = ms.clone();
		let watch = move |path: &str| {
			let (tx, rx) = std::sync::mpsc::channel();

			use notify::Watcher;
			let mut watcher = notify::RecommendedWatcher::new(tx, notify::Config::default()).unwrap();

			// Add a path to be watched. All files and directories at that path and
			// below will be monitored for changes.
			// ....nonrecursive does the same thing as recursive but whatever....
			watcher.watch(path.as_ref(), notify::RecursiveMode::NonRecursive).unwrap();

			let mut event_count = 0;
			for res in rx {
				match res {
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
				}
			}
		};

		// watch plugin file changes
		std::thread::spawn(move || {
			watch(&*PLUGIN_PATH);
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

	let set_active_func = |mut ms: MutexGuard<MutState>, n| match ms.plugins.len().cmp(&n) {
		std::cmp::Ordering::Less => eprintln!("plugin {n} not loaded"),
		_ => ms.active_func = n,
	};

	match key {
		Key::R => ms.is_reset = true,

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

fn update(_app: &App, state: &mut State, _update: Update) {
	let mut buffer = [0.0; 1024];
	state.consumer.pop_slice(&mut buffer);
	let mut ap = state.audio_processor.lock().unwrap();
	ap.add_samples(&buffer);
}


fn view(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let mut ms = s.ms.lock().unwrap();
	let ap = s.audio_processor.lock().unwrap();

	let mags = ap.get_magnitudes();

	// a pretty good decay factor
	// can be controlled by midi but here for reference
	// should give a slow smeary like feeling
	// const FACTOR: f32 = 0.9999;

	// TODO: smoothing somewhere??
	// apply the smoothed values to the fft_buf
	// fft.iter().map(|(_, x)| x)
	// 	.zip(fft_buf.iter_mut()).for_each(|(c, p)| 
	// 		if *c > *p { *p = *c; } 
	// 		else { *p *= FACTOR; });
	
	static mut TIME: f32 = 0.0;

	const UPPER_TIME_LIMIT: f32 = 524288.0;
	const LOWER_TIME_LIMIT: f32 = -524288.0;
	if unsafe { TIME >= UPPER_TIME_LIMIT || TIME <= LOWER_TIME_LIMIT } {
		ms.is_backwards = !ms.is_backwards;
	}
	
	let mut i = 0;
	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
	{
		i += 1;
		if i == mags.len() {
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

		// TODO: dynamic value for luminance changed on a function?
		// let sat = ms.plugins[ms.active_func].call(r.x(), r.y(), t, mags);
		let sat = if ms.is_fft {
			midi::lerp_float((mags[i] + ms.sat_mod).ceil() as u8, 0.01, 0.6, 0, 100)
		} else { 0.5 };

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(hue, 1.0, sat);
	}

	draw.to_frame(app, &frame).unwrap();
}

