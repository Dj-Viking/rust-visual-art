use portmidi as pm;
use nannou::prelude::*;

use spectrum_analyzer::windows::hann_window;
use spectrum_analyzer::{
	FrequencySpectrum, 
	samples_fft_to_spectrum, 
	FrequencyLimit
};
use spectrum_analyzer::scaling::divide_by_N_sqrt;

use byteorder::ReadBytesExt;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::{
	ffi::CString,
	io::{BufReader, Read},
	os::unix::net::UnixStream,
};

#[derive(Debug, Clone, PartialEq, Copy, Default)]
#[repr(u8)]
enum ActiveFunc {
	#[default]
	Spiral = 0,
	V2     = 1,
	Waves  = 2,
	Solid  = 3,
	Audio  = 4,
}

struct State {
	funcs:        &'static [fn(f32, f32, f32, Option<&FrequencySpectrum>) -> f32],
	ms:           Arc<Mutex<MutState>>,
	audio_info:   pulseaudio::protocol::SourceInfo,
}

static mut AUDIO_STATE: Vec<f32> = Vec::<f32>::new();

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
	solid:          u8,
	audio:          u8,
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
		let ms_ = ms.clone();

		let pm_ctx = pm::PortMidi::new().unwrap();
		let devices = pm_ctx.devices().unwrap();

		// setup audio stream
		let (mut sock, protocol_version) =
			connect_and_init();

		// hard coded device name found with
		// running `pacmd list-sources`
		let device_name = "alsa_input.usb-Yamaha_Corporation_Yamaha_AG06MK2-00.analog-stereo";
		
		pulseaudio::protocol::write_command_message(
			sock.get_mut(),
			10,
			pulseaudio::protocol::Command::GetSourceInfo(pulseaudio::protocol::GetSourceInfo {
				name: Some(CString::new(&*device_name).unwrap()),
				..Default::default()
			}),
			protocol_version,
		).unwrap();

		let (_, source_info) =
			pulseaudio::protocol::read_reply_message::<pulseaudio::protocol::SourceInfo>(
				&mut sock, protocol_version
			).unwrap();

		println!("audio socket {:#?}", sock);

		// make recording stream on the server
		pulseaudio::protocol::write_command_message(
			sock.get_mut(),
			99,
			pulseaudio::protocol::Command::CreateRecordStream(
				pulseaudio::protocol::RecordStreamParams {
					source_index: Some(source_info.index),
					sample_spec: pulseaudio::protocol::SampleSpec {
						format: source_info.sample_spec.format,
						channels: source_info.channel_map.num_channels(),
						sample_rate: source_info.sample_spec.sample_rate,
					},
					channel_map: source_info.channel_map,
					cvolume: Some(pulseaudio::protocol::ChannelVolume::norm(2)),
					..Default::default()
				}
			),
			protocol_version,
		).unwrap();

		let (_, record_stream) =
			pulseaudio::protocol::read_reply_message::<pulseaudio::protocol::CreateRecordStreamReply>(
			&mut sock,
			protocol_version,
		).unwrap();

		// buffer for the audio samples
		let mut buf = vec![0; record_stream.buffer_attr.fragment_size as usize];
		let mut float_buf = Vec::<f32>::new();

		println!("record strim reply {:#?}", record_stream);
		
		// similar loop for audio here?
		// in different thread to keep updating the float_buf and
		// pass that buf into the state to get fft calculated on each frame maybe?
		std::thread::spawn(move || {
			//static mut BACKOFF: u8 = 0;

			loop {
				// lazy backoff for now...
				std::thread::sleep(std::time::Duration::from_millis(1));

				// let mut audio_state = audio_state_.lock().unwrap();
				let descriptor = pulseaudio::protocol::read_descriptor(
					&mut sock
				).unwrap();
				// channel of -1 is a command message. everything else is data
				if descriptor.channel == u32::MAX {
					let (_, msg) = pulseaudio::protocol::Command::read_tag_prefixed(
						&mut sock,
						protocol_version,
					).unwrap();
					println!("message from server when channel was u32 max ?? {:?}", msg);
				} else {
					buf.resize(descriptor.length as usize, 0);
					unsafe { AUDIO_STATE.clear(); }

					// read socket data
					sock.read_exact(&mut buf).unwrap();
					let mut cursor = std::io::Cursor::new(buf.as_slice());
					while cursor.position() < cursor.get_ref().len() as u64 {
						match record_stream.sample_spec.format {
							pulseaudio::protocol::SampleFormat::S32Le => {
								let sample = cursor.read_i32::<byteorder::LittleEndian>().unwrap();
								unsafe { AUDIO_STATE.push(sample as f32); }
							},
							_ => unreachable!(),
						}
					}
				}
			}
		});

		// audio setup end 
		
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

		std::thread::spawn(move || {
			let mut in_port = pm_ctx.input_port(dev, 256).unwrap();

			loop {
				static mut BACKOFF: u8 = 0;
				// TODO: listen flag

				let Ok(Some(m)) = in_port.read() else {
					std::hint::spin_loop();

					std::thread::sleep(
						std::time::Duration::from_millis(
							unsafe { BACKOFF * 10 } as u64
						)
					);

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

				if channel == cfg.solid && intensity > 0 {
					ms.func = ActiveFunc::Solid;
				}

				if channel == cfg.audio && intensity > 0 {
					ms.func = ActiveFunc::Audio;
				}

				ms.is_reset = channel == cfg.reset && intensity > 0;

				if channel == cfg.backwards && intensity > 0 {
					ms.is_backwards = !ms.is_backwards;
				}

				unsafe { BACKOFF = 0; }
			}
		});

		a.new_window()
			.view(view)
			.build().unwrap();

		State {
			ms,
			audio_info: source_info,
			funcs: &[
				|y, x, t, _| y * x * t, // spiral
				|y, x, t, _| 32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05), // v2
				|y, x, t, _| x / y * t, // waves
				|y, x, t, _| (x % 2.0 + 1000.0) / (y % 2.0 + 1000.0) * (t), // solid
				|y, x, t, fft_data| { // audio
					let mut y_ = y.clone();
					let mut x_ = x.clone();
					let mut t_ = t.clone();
					// what to do here??
					// the app got a lot slower now :( but maybe on the right track?
					if let Some(fft) = fft_data {
						for (fr, fr_val) in fft_data.unwrap().data().iter() {
							if fr.val() < 500.0 {
								if fr_val.val() > 100.0 {
									t_ += 100.0;
								}
							} else {

							}
						}
						y_ * x_ * t_
					} else {
						y * x * t
					}
				}
			],
		}
	};

	nannou::app(init).run();
}

fn view(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);
	let mut ms = s.ms.lock().unwrap();
	static mut spectrum_fft_data: Option<FrequencySpectrum> = None;
	
	unsafe {
		if !(&AUDIO_STATE.len() < &256) {

			let hann_window = hann_window(&AUDIO_STATE[0..256]);

			spectrum_fft_data = Some(samples_fft_to_spectrum(
				&hann_window,
				s.audio_info.sample_spec.sample_rate,
				FrequencyLimit::Range(50.0, 12000.0),
				Some(&divide_by_N_sqrt),
			).unwrap());
		}
	}

	//print!("\x1B[2J\x1B[1;1H");
	// if let Some(ref s) = spectrum_fft_data {
	// 	for (fr, fr_val) in spectrum_fft_data.unwrap().data().iter() {
	// 		if fr.val() < 500.0 {
	// 			println!("{:<10}Hz => {}", fr.to_string(), ".".repeat((fr_val.val() / 10000000.0) as usize));
	// 		} else {
	// 			println!("{:<10}Hz => {}", fr.to_string(), ".".repeat((fr_val.val() / (1000000.0) ) as usize));
	// 		}
	// 	}
	// }

	static mut TIME: f32 = 0.0;
	let mut value: f32 = 0.0;

	unsafe{
		value = s.funcs[ms.func as u8 as usize](1.0, 1.0, 1.0, spectrum_fft_data.as_ref());
	}
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
			(time_divisor + 100000.0 * (ms.time_dialiation as f32 / 10.0))
			+ ms.current_intensity as f32 / 100.0;

		unsafe {
			value = s.funcs[ms.func as u8 as usize](r.y(), r.x(), t, spectrum_fft_data.as_ref());
		}
		let hue = value;

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(hue, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}

// establish an audio client for the pulseaudio server
fn connect_and_init() -> (BufReader<UnixStream>, u16) {

    let socket_path = pulseaudio::socket_path_from_env().unwrap();
    let mut sock = std::io::BufReader::new(UnixStream::connect(socket_path).unwrap());

    let cookie = pulseaudio::cookie_path_from_env()
        .and_then(|path| std::fs::read(path).ok())
        .unwrap_or_default();
    let auth = pulseaudio::protocol::AuthParams {
        version: pulseaudio::protocol::MAX_VERSION,
        supports_shm: false,
        supports_memfd: false,
        cookie,
    };

    pulseaudio::protocol::write_command_message(
        sock.get_mut(),
        0,
        pulseaudio::protocol::Command::Auth(auth),
        pulseaudio::protocol::MAX_VERSION,
    ).unwrap();

    let (_, auth_reply) =
        pulseaudio::protocol::read_reply_message::<pulseaudio::protocol::AuthReply>(
			&mut sock, pulseaudio::protocol::MAX_VERSION
		).unwrap();
    let protocol_version = std::cmp::min(
		pulseaudio::protocol::MAX_VERSION, auth_reply.version
	);

    let mut props = pulseaudio::protocol::Props::new();
    props.set(
        pulseaudio::protocol::Prop::ApplicationName,
        CString::new("pulseaudio-rs-playback").unwrap(),
    );

    pulseaudio::protocol::write_command_message(
        sock.get_mut(),
        1,
        pulseaudio::protocol::Command::SetClientName(props),
        protocol_version,
    ).unwrap();

    let _ =
        pulseaudio::protocol::read_reply_message::<pulseaudio::protocol::SetClientNameReply>(
			&mut sock, protocol_version
		).unwrap();

    (sock, protocol_version)
}
