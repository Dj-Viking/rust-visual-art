use std::io::Write;

const CLEAR: &[u8] = b"\x1b[2J\x1b[1;1H";
const RESET: &str = "\x1b[0m";

fn main() {
	let mut out = std::io::stdout();	
	out.write_all(CLEAR).unwrap();

	const FPS: u64 = 10;

	const WIDTH: usize = 180;
	const HEIGHT: usize = 40;

	let spiral_pattern = |col: usize, row: usize, frame: usize, adjust: usize|
		(20, 10, (col * (frame * adjust) * row * (adjust * 2)) as u8);

	let color_to_escape = |color: (u8, u8, u8)|
		format!("\x1b[38;2;{};{};{}m", color.0, color.1, color.2);

	let mut ptime = std::time::Instant::now();
	let mut frame = 0;

	loop {
		frame += 1;
		std::thread::sleep(std::time::Duration::from_millis(1000 / FPS - ptime.elapsed().as_millis() as u64));
		// let delta = ptime.elapsed().as_millis(); // ACTUAL Δt
		ptime = std::time::Instant::now();

		out.write_all(CLEAR).unwrap();
		(0..HEIGHT).for_each(|c| {
			(0..WIDTH).for_each(|r| { 
				out.write_all(format!("{}█{RESET}", 
					color_to_escape(spiral_pattern(c, r, frame, 1))).as_bytes()).unwrap(); 
			});
			out.write_all(b"\n").unwrap();
		});

		out.flush().unwrap();
	}
}
