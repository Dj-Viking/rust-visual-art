use std::sync::LazyLock;

pub static ARGS: LazyLock<Args> = LazyLock::new(|| {
	let mut out = Args::default();

	for arg in std::env::args().skip(1) {
		match arg.as_str() {
			"list" => {
				let pm_ctx = portmidi::PortMidi::new().unwrap();
				let devices = pm_ctx.devices().unwrap();
				devices.iter().for_each(|d| println!("[MAIN]: device {} {:?} {:?}", d.id(), d.name(), d.direction()));
				std::process::exit(0);
			},
			"hmr"       => out.hmr_enable = true,
			"logupdate" => out.log_update = true,
			_ => { },
		}
	}

	out
});

#[derive(Default, Debug)]
pub struct Args {
	pub hmr_enable: bool,
	pub log_update: bool,
}
