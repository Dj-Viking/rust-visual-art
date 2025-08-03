#![allow(static_mut_refs)]
#![allow(unused_imports)]

use portmidi::PortMidi;

use nannou::prelude::*;
use nannou_audio;
use nannou_audio::Buffer;

use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::collections::HashMap;

use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;

mod midi;
mod loading;
mod audio_processor;
mod utils;

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
	Solid,
	Something
}

struct State {
	ms:              Arc<Mutex<MutState>>,
	consumer:        ringbuf::HeapCons<f32>,
	audio_processor: Arc<Mutex<audio_processor::AudioProcessor>>,
}

#[derive(Default, Debug)]
struct MutState {
	is_backwards:       bool,
	is_reset:           bool,
	is_saving_preset:   bool,
	is_listening_midi:  bool,
	is_listening_keys:  bool,
	plugins:            Vec<loading::Plugin>,
	save_state:         SaveState,
	user_cc_map:        HashMap<String, SaveState>,
	midi_config_fn_ccs: Vec<u8>, // assigned by the user
	well_known_ccs:     Vec<u8>, // assigned in the midi config.toml
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SaveState {
	cc:                u8,
	active_func:       usize,
	is_fft:            bool,
	current_intensity: f32,
	time_dialation:    f32,
	decay_factor:      f32,
	lum_mod:           f32,
	modulo_param:      f32,
	decay_param:       f32,
}
impl SaveState {
	fn new(ms: &mut MutexGuard<'_, MutState>) -> Self {
		Self {
			cc:                ms.save_state.cc,
			active_func:       ms.save_state.active_func,
			is_fft:            ms.save_state.is_fft,
			current_intensity: ms.save_state.current_intensity,
			time_dialation:    ms.save_state.time_dialation,
			decay_factor:      ms.save_state.decay_factor,
			lum_mod:           ms.save_state.lum_mod,
			modulo_param:      ms.save_state.modulo_param,
			decay_param:       ms.save_state.decay_param,
		}
	}
}

static USER_SS_CONFIG: LazyLock<&'static str> =
	LazyLock::new(|| std::env::var("USER_SS_CONFIG_PATH")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("user_ss_config"));

static SAVE_STATES:    LazyLock<&'static str> =
	LazyLock::new(|| std::env::var("SAVE_STATES_PATH")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("default_user_save_state.toml"));

static CONF_FILE:      LazyLock<&'static str> =
	LazyLock::new(|| std::env::var("CONF_FILE_PATH")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("config.toml"));

static PLUGIN_PATH:    LazyLock<&'static str> =
	LazyLock::new(|| std::env::var("PLUGIN_PATH")
		.map(|s| &*Box::leak(s.into_boxed_str()))
		.unwrap_or("target/libs"));

const SAMPLES: usize = 4096;
static mut PLUGS_COUNT: u8 = 0;

static mut HMR_ON: bool = false;

// if enabled then fill the MutState with the user_cc_map from their own toml file

pub static mut LOGUPDATE: bool = false;


fn use_user_defined_cc_mappings () 
	-> Result<
		(HashMap<String, SaveState>, Vec<u8>), 
		Box<dyn std::error::Error> >
{
	let mut hm = HashMap::<String, SaveState>::new();
	// read dir for all files
	// parse all files into a hashmap with hashkeys are the cc number for the mapping
	// and the values are the SaveState structs
	
	std::fs::read_dir(*crate::USER_SS_CONFIG)
		// I'll get more than one path here if there's more
		// so then more key values would be inserted to this hashmap
		?.into_iter().for_each(|path| {
			//parse the toml at the dir path	
			let config: HashMap<String, SaveState> = 
				toml::from_str(
					&std::fs::read_to_string(path.unwrap().path()).unwrap()
				).unwrap_or_else(|e| {
					eprintln!("[MAIN]: Error reading save_state file: {e}");
					std::process::exit(1);
				});
			
			hm.insert(
				config.keys().last().unwrap().to_string(),
				config.values().last().unwrap().clone()
			);
		});

	println!("[MAIN]: user ss config cc map {:#?}", hm);

	let hmkeys = hm
			.keys().into_iter()
			.map(|k| k.parse::<u8>().unwrap())
			.collect::<Vec<u8>>();

	Ok((hm, hmkeys))
}


fn check_args() {

	// list midi devices in terminal
	if std::env::args().skip(1).any(|a| a == "list") {
		let pm_ctx = PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();
		devices.iter().for_each(|d| println!("[MAIN]: devices {} {:?} {:?}", d.id(), d.name(), d.direction()));
		std::process::exit(0);
	}

	if std::env::args().skip(1).any(|a| a == "hmr") {
		unsafe { HMR_ON = true; };
	}


	if std::env::args().skip(1).any(|a| a == "logupdate") {
		unsafe { LOGUPDATE = true; };
	}
}

fn use_default_user_save_state(ss_map: &HashMap<String, SaveState>) -> Option<SaveState> {
	if unsafe { !HMR_ON } {
		return Some(SaveState {
			cc:                ss_map["0"].cc,
			active_func:       ss_map["0"].active_func,
			is_fft:            ss_map["0"].is_fft,
			current_intensity: ss_map["0"].current_intensity,
			time_dialation:    ss_map["0"].time_dialation,
			decay_factor:      ss_map["0"].decay_factor,
			lum_mod:           ss_map["0"].lum_mod,
			modulo_param:      ss_map["0"].modulo_param,
			decay_param:       ss_map["0"].decay_param,
		});
	}

	None
}

fn check_libs() {
	// get the amount of plugins as part of knowing when to reload the m
	// when the watcher detects changes
	if let Ok(entries) = std::fs::read_dir(&*PLUGIN_PATH) {
		for entry in entries {
			if let Ok(_) = entry {
				unsafe { PLUGS_COUNT += 1 };
			}
		}

		println!("[MAIN]: plug count {}", unsafe { PLUGS_COUNT });
	} else {
		println!("[MAIN]: [info]: no target/libs dir existed...");
		println!("[MAIN]: [info]: recompiling...");
		let status = std::process::Command::new("./build_script/target/debug/build_script")
			.output()
			.unwrap();
		println!("[MAIN]: [info]: {}", std::str::from_utf8(&status.stdout).unwrap());
		println!("[MAIN]: [error]: {}", std::str::from_utf8(&status.stderr).unwrap());
		assert!(status.status.success());
	}

}

fn main() {

	check_args();
	check_libs();

	let init = |a: &App| {

		// letting this fail because I don't want main necessarily to be wrapped in a result yet
		// if this fails that means you don't have a controller plugged in

		let ss_map: HashMap<String, SaveState> =
			toml::from_str(&std::fs::read_to_string(*crate::SAVE_STATES).unwrap()).unwrap_or_else(|e| {
				eprintln!("[MAIN]: Error reading save_states.toml file: {e}");
				std::process::exit(1);
			});

		let pm_ctx = PortMidi::new().unwrap();
		let midi = midi::Midi::new(&pm_ctx).unwrap();

		let res = use_user_defined_cc_mappings();

		let midi_ccs = utils::get_midi_ccs(&pm_ctx).unwrap();

		let ms = Arc::new(Mutex::new(MutState {
			is_listening_keys: false,
			is_listening_midi: false,
			plugins: {
				let mut p = Vec::new();
				loading::Plugin::load_dir(*PLUGIN_PATH, &mut p);
				p
			},
			// loading in the save state if we did not pass 'hmr' as a cli arg to cargo
			save_state: { 
				// hot reloading not that useful in this configuration
				// more for using midi controller to switch to preset visual patches with their
				// user_cc_config
				if let Some(ss) = use_default_user_save_state(&ss_map) {
					println!("[MAIN]: using defined default save state {:#?}", ss_map);
					ss
				} else {
						// hot reloading is only noticable in this configuration
					let dss = SaveState {..Default::default()};
					println!("[MAIN]: not using defined save state... using default: {:#?}", dss); 
					dss
				}
			},
			well_known_ccs: midi_ccs,
			user_cc_map: if let Ok((ref hm, _)) = res { hm.clone() } else {
				let dss = SaveState {..Default::default()};
				let mut hm = HashMap::<String, SaveState>::new();
				hm.insert("0".to_string(), dss);
				hm
			} ,
			midi_config_fn_ccs: if let Ok((_, cckeys)) = res { cckeys } else {
				vec![]
			},
			..Default::default()
		}));

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
			// in a loop?? i dont remember
			// out_stream.play().unwrap();
		});

		let ms_ = ms.clone();
		if !std::env::args().skip(1).any(|arg| arg == "keys") {
			// can't easily move this block :(
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


		let ms_ = ms.clone();
		// watch plugin file changes if user passed hmr as a cli arg
		if unsafe { HMR_ON } {
			std::thread::spawn(move || {
				watch(&*PLUGIN_PATH, &ms_);
			});
		}

		a.new_window()
			.view(view)
			.key_pressed(key_pressed)
			.key_released(key_released)
			.build().unwrap();

		let state = State {
			ms,
			consumer: cons,
			audio_processor,
		};

		println!("state.ms mutstate {:#?}", state.ms);

		state
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
		Key::R => ms.is_reset         = false,
		Key::P => { println!("[MAIN][UTILS]: is_saving_preset false");
			      ms.is_saving_preset = false;
		},
		_ => ()
	}
}

fn key_pressed(_: &App, s: &mut State, key: Key) {
	let mut ms = s.ms.lock().unwrap();

	let set_active_func = |mut ms: MutexGuard<MutState>, n: ActiveFunc| {
		println!("[MAIN]: active func {:?}", ms.save_state.active_func);
		match ms.plugins.len().cmp(&(n.clone() as usize)) {
			std::cmp::Ordering::Less => eprintln!("[MAIN]: plugin {:?} not loaded", n),
			_ => ms.save_state.active_func = n as usize,
		}
	};

	match key {
		Key::R => ms.is_reset    = true,
		Key::P => {
			println!("[Main]: is_saving_preset true");
			ms.is_saving_preset  = true;
		},
		Key::L => {
			println!("[MAIN][KEYS]: is_listening_keys true");
			ms.is_listening_keys = true;
		}

		Key::Key1 => set_active_func(ms, ActiveFunc::Spiral),
		Key::Key2 => set_active_func(ms, ActiveFunc::V2),
		Key::Key3 => set_active_func(ms, ActiveFunc::Waves),
		Key::Key4 => set_active_func(ms, ActiveFunc::Audio),
		Key::Key5 => set_active_func(ms, ActiveFunc::Solid),
		Key::Key6 => set_active_func(ms, ActiveFunc::Something),

		Key::Up    if ms.save_state.current_intensity < 255.0 => ms.save_state.current_intensity += 1.0,
		Key::Down  if ms.save_state.current_intensity > 0.0   => ms.save_state.current_intensity -= 1.0,
		Key::Right if ms.save_state.time_dialation    < 255.0 => ms.save_state.time_dialation    += 1.0,
		Key::Left  if ms.save_state.time_dialation    > 0.0   => ms.save_state.time_dialation    -= 1.0,

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

	if ms.is_listening_midi {
		utils::handle_save_preset_midi(&mut ms);
	} else {
		// no other thing listening for saving
		// just keys here
		utils::handle_save_preset_keys(&mut ms);
	}

	// println!("[MAIN]: ============");
	// f32::memprint_block(&ap.buffer);
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
		if ms.plugins.len() != 0 {
			let t: f32 = unsafe { TIME } / (
					ms.plugins[ms.save_state.active_func].time_divisor
					+ TIME_OFFSET
					* (ms.save_state.time_dialation / 10.0)
				)
				+ ms.save_state.current_intensity / 100.0;

			hue = ms.plugins[ms.save_state.active_func].call(r.x(), r.y(), t);
		}



		let lum = if ms.save_state.is_fft {
			lerp_float((mags[i as usize] + ms.save_state.lum_mod).ceil() as u8, 0.01, 0.6, 0, 100)
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

				let lib_name = event.paths[0]
					.to_str().unwrap()
					.split("/")
					.last().unwrap();

				event_count += 1;

				println!("[MAIN]: lib removed: {:?}", lib_name);
				// wait for files to be fully removed
				// and recreated by the build script
				// kind of a weird hack because an event is fired for each File
				// in the plugin path but I need to wait for all the files
				// to be replaced
				if event_count == unsafe { PLUGS_COUNT * 4 } {

					println!("[MAIN]: event count {:?}", event_count);

					let mut ms = ms_.lock().unwrap();

					println!("[MAIN]: \n=========\n watch event: {:?}", event.kind);

					event_count = 0;

					println!("[MAIN]: [INFO]: reloading plugins");
					std::thread::sleep(
						std::time::Duration::from_millis(
							100));
					ms.plugins.clear();
					loading::Plugin::load_dir(*PLUGIN_PATH, &mut ms.plugins);
				}
			}
		},
		Err(error) => println!("[MAIN]: Error: {:?}", error),
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
