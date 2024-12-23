use nannou::prelude::*;

struct State {
	finx:  usize,
	funcs: Vec<fn(f32, f32, f32) -> f32>,
}

fn main() {
	let init = |a: &App| { 
		let spiral = |y: f32, x: f32, t: f32| y * t * x;
		let v2 = |y: f32, x: f32, t: f32| 32.0 / (t / x) + y / (x / y - 1.0 / t) + t * (y * 0.05);

		a.new_window()
			.view(update)
			.key_pressed(key)
			.build().unwrap(); 

		State {
			finx: 0,
			funcs: vec![spiral, v2],
		}
	};

	nannou::app(init).run();
}

fn key(_: &App, s: &mut State, key: Key) {
	match key {
		Key::Space => s.finx = (s.finx + 1) % s.funcs.len(),
		_ => (),
	}
}

fn update(app: &App, s: &State, frame: Frame) {
	let draw = app.draw();
	draw.background().color(BLACK);

	let f = s.funcs[s.finx];
	let t = app.time / 100000.0;

	for r in app.window_rect().subdivisions_iter()
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter())
		.flat_map(|r| r.subdivisions_iter()) {
		let sat = f(r.y(), r.x(), t);

		draw.rect().xy(r.xy()).wh(r.wh())
			.hsl(sat, 1.0, 0.5);
	}

	draw.to_frame(app, &frame).unwrap();
}
