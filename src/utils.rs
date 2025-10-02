use std::sync::Mutex;

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

pub fn watch(plugs_count: u8, path: &str, ms_: &std::sync::Arc<Mutex<crate::MutState>>) {
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
					if event_count == plugs_count * 4 {
						println!("[MAIN]: event count {:?}", event_count);

						let mut ms = ms_.lock().unwrap();

						println!("[MAIN]: \n=========\n watch event: {:?}", event.kind);

						event_count = 0;

						println!("[MAIN]: [INFO]: reloading plugins");
						std::thread::sleep(std::time::Duration::from_millis(100));
						ms.plugins.clear();
						crate::loading::Plugin::load_dir(&*crate::PLUGIN_PATH, &mut ms.plugins);
					}
				}
			},
			Err(error) => println!("[MAIN]: Error: {:?}", error),
		}
	}
}
