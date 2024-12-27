use portmidi as pm;
use nannou::prelude::*;
use std::time::Duration;
use std::thread;

static mut TIME_NOW: f32 = 0.0;
const TIME_DIVISOR: f32 = 1000000000.0;

#[derive(Debug)]
struct MidiState {
	current_channel: u8,
	current_intensity: u8,
	intensity_channel: u8,
	time_dialation_channel: u8,
	timeout: Duration 
}

const fn new_midi_state() -> MidiState {
	MidiState {
		current_channel: 0,
		current_intensity: 0,
		time_dialation_channel: 0,
		intensity_channel: 0,
		timeout: Duration::from_millis(10),
	}
}

static mut MS: MidiState = new_midi_state();

struct State {
	finx:  usize,
	reset: bool,
	funcs: Vec<fn(f32, f32, f32) -> f32>,
}

// TODO: figure out how to dynamically get the controller I want to use
// from the config file and all it's mappings
// for now only mapped up to XONE controller
// the format is hard coded for now
fn read_midi_input_config() -> () {
	let text = std::fs::read_to_string(".midi-input-config").unwrap()
		.split('\n')
		.filter(|l| !l.is_empty())
		.map(|l| l.to_string())
		.collect::<Vec<String>>();

	for i in 0..text.clone().into_iter().len() {
		println!("line {}", text[i]);
		// hard coded known format only 
		// two entries below the XONE label in the config file for now
		if text[i].contains("[XONE]") {
			unsafe {
				MS.intensity_channel = text[i + 1]
					.split('=')
					.collect::<Vec<&str>>()[1]
					.parse::<u8>().unwrap();
				MS.time_dialation_channel = text[i + 2]
					.split('=')
					.collect::<Vec<&str>>()[1]
					.parse::<u8>().unwrap();
			}
		}
	}
}

fn main() {

	let init = |a: &App| { 

		let pm_ctx = pm::PortMidi::new().unwrap();
		let xone_id = get_xonek2_id(&pm_ctx);
		let info = pm_ctx.device(xone_id).unwrap();

		// map the channels to which part of the effect to control
		read_midi_input_config();

		thread::spawn(move || {
			let in_port = pm_ctx.input_port(info, 1024)
				.unwrap();
			while let Ok(_) = in_port.poll() {
				if let Ok(Some(m)) = in_port.read_n(1024) {
					handle_midi_msg(MyMidiMessage::new(m[0]));
				}
			}
			unsafe {
				thread::sleep(MS.timeout);
			}
		});

		let spiral = |y: f32, x: f32, t: f32| y * t * x;
		let v2 = |y: f32, x: f32, t: f32| 32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05);

		a.new_window()
			.view(update)
			.key_pressed(key_pressed)
			.key_released(key_released)
			.build().unwrap(); 

		State {
			finx: 0,
			reset: false,
			funcs: vec![spiral, v2],
		}
	};

	nannou::app(init).run();
}

#[derive(Debug)]
struct MyMidiMessage {
	channel: u8,
	intensity: u8,
}
impl MyMidiMessage {
	fn new(m: pm::types::MidiEvent) -> Self {
		Self {
			channel: m.message.data1,
			intensity: m.message.data2,
		}
	}
}

fn handle_midi_msg(m: MyMidiMessage) -> () {
	unsafe {
		MS.current_channel = m.channel;

		if listen_midi_channel(m.channel, MS.intensity_channel) {
			MS.current_intensity = m.intensity;
		}

	}
}

fn get_xonek2_id(pm: &pm::PortMidi) -> i32 {
	let mut ret: i32 = 0;
	for d in pm.devices().unwrap() {
		if d.name().contains("XONE") {
			ret = d.id();
		}
	}
	ret
}

fn key_released(_: &App, s: &mut State, key: Key) {
	match key {
		Key::Tab => s.reset = false,
		_ => (),
	}
}
fn key_pressed(_: &App, s: &mut State, key: Key) {
	match key {
		Key::Space => s.finx = (s.finx + 1) % s.funcs.len(),
		Key::Tab => s.reset = true,
		_ => (),
	}
}

fn listen_midi_channel(in_channel: u8, channel: u8) -> bool {
	if in_channel == channel {
		return true;
	}
	return false;
}

fn update(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);

	let f = s.funcs[s.finx];


	let t = |s: &State| {
		unsafe {
			TIME_NOW += app.duration.since_prev_update.as_secs_f32();
			if s.reset {
				TIME_NOW = 0.0; 
			}

			return TIME_NOW / TIME_DIVISOR + (MS.current_intensity as f32 / 100.0) as f32;
		}
	};

	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
	{
		let hue = f(r.y(), r.x(), t(s));

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(hue, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}
