use portmidi as pm;
use nannou::prelude::*;
use std::time::Duration;
use std::thread;

static mut TIME_NOW: f32 = 0.0;
static mut INTENSITY: u8 = 0;
const TIME_DIVISOR: f32 = 1000000000.0;
const TIMEOUT: Duration = Duration::from_millis(10);

struct State {
	finx:  usize,
	reset: bool,
	funcs: Vec<fn(f32, f32, f32) -> f32>,
}

fn main() {

	let init = |a: &App| { 
		let pm_ctx = pm::PortMidi::new().unwrap();
		let xone_id = get_xonek2_id(&pm_ctx);
		let info = pm_ctx.device(xone_id).unwrap();

		thread::spawn(move || {
			let in_port = pm_ctx.input_port(info, 1024)
				.unwrap();
			while let Ok(_) = in_port.poll() {
				if let Ok(Some(m)) = in_port.read_n(1024) {
					handle_midi_msg(MyMidiMessage::new(m[0]));
				}
			}
			thread::sleep(TIMEOUT);
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
	println!("{:?}", m);
	unsafe {
		INTENSITY = m.intensity;
		println!("{}", INTENSITY);
	}
}

fn get_xonek2_id(pm: &pm::PortMidi) -> i32 {
	let mut ret = 0;
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
			// todo: use different values other than intensity to adjust how the 
			// visuals change based on user input 
			return TIME_NOW / TIME_DIVISOR + (INTENSITY as f32 / 100.0) as f32;
		}
	};

	//println!("what is t {}", t(s));

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
