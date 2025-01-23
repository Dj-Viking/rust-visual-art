// example https://github.com/PauSala/fftvisualizer/tree/main
use portmidi as pm;
use nannou::prelude::*;

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

		a.new_window()
			.view(view)
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

fn view(app: &App, s: &State, frame: Frame) {
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
			(time_divisor + 100000.0 * (ms.time_dialiation as f32 / 10.0))
			+ ms.current_intensity as f32 / 100.0;

		let hue = s.funcs[ms.func as u8 as usize](r.y(), r.x(), t);

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
