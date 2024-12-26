use nannou::prelude::*;

struct State {
	finx:  usize,
	reset: bool,
	funcs: Vec<fn(f32, f32, f32) -> f32>,
}

fn main() {
	let init = |a: &App| { 
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

static mut TIME_NOW: f32 = 0.0;

fn update(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);

	let f = s.funcs[s.finx];

	let t = |_: &State| {
		unsafe {
			TIME_NOW += app.duration.since_prev_update.as_secs_f32();
			if s.reset {
				TIME_NOW = 0.0; 
			}
			return TIME_NOW / 1000000000.0;
		}
	};

	println!("what is t {}", t(s));

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
