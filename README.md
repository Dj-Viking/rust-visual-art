# rust-visual-art

<div style="display: flex; justify-content: space-between; flex-direction: row"> 
	<img height="320" width="180" src="./readme-examples/2025-03-31_15-59.png"/>
	<img height="320" width="180" src="./readme-examples/2025-03-31_15-58.png"/>
	<img height="320" width="180" src="./readme-examples/2025-03-31_15-58_1.png"/>
</div>

---

### Running the app
You can run in either release mode ( best option ) 
```sh
cargo run --release
```
or debug mode
```sh
cargo run
```

### hot-reloading
To set up the hot-reloading - run this command in a separate shell
to watch the changes to lib/*.rs which are the "plugin" functions 
that control the visuals

NOTE: requires cargo watch - install with `cargo install cargo-watch`

```sh
sh watch.sh
```

for best results run in release mode
```sh
cargo run --release
```

---
## Dependencies
### midi

For now, in order to build and run this project; 
you'll need to install the portmidi library through your package manager.

I built this on arch linux so it would be like so:

```sh
sudo pacman --sync portmidi
```

---
### keyboard
However if you don't wish to use a midi controller at the moment 
and just use the keyboard by starting the application with `keys` as a command line argument
to cargo.

```sh
cargo run keys
# or release
cargo run --release keys
```

And the visual patches can be changed currently by the number keys

The names of the patches with their numbers respectively. Which match up to the dynamically loaded libs
by the file names prefixed with the number. This makes sure that the libs are loaded in the same order
each time they are hot reloaded if the library code changes during runtime.


* Spiral - `1`,
* V2     - `2`,
* Waves  - `3`,
* Audio  - `4`,
* Solid  - `5`

And the effect parameters currently setup can be changed with the arrow keys

* current_intensity modifier - `KeyUp`(positive direction),`KeyDown`(negative direction)
* time_dialation modifier    - `KeyLeft`(negative direction),`KeyRight`(positive direction)

---
### audio

The audio mechanism used in this project utilizes nannou_audio and took inspiration from [this example](https://github.com/nannou-org/nannou/blob/bevy-refactor/examples/audio/feedback.rs) 
which Josh Batty kindly updated recently on the `bevy-refactor` branch of nannou.

As well as inspiration from Lokuas lattice project which makes use of audio reactivity [here](https://github.com/Lokua/lattice/blob/main/src/framework/audio.rs)

nannou_audio essentially which wraps around `cpal` under the hood
which interestingly **will use the "default"** device of the user's system.

Which on my arch setup, the default device, is pipewire. But in order for the rest of my system to work normally I needed to adjust
my setup slightly. 

Since removing pulseaudio - I needed to install `pipewire-pulse` for other software that
depended on pulseaudio before to continue to work properly with pipewire instead.

And then to adjust any settings within `pwvucontrol` (yes that is spelled right) the pipewire equivalent of pavucontrol.
I needed to also install `wireplumber`.

Once that was all installed I needed to make sure everything was running properly on my system so that means
configuring my system on startup to run `pipewire` `pipewire-pulse` and `wireplumber`.

`rust-visual-art` will then start up and become a recording source appearing inside `pwvucontrol` and will receive input
from whichever input device is **set by the user as default**. This is IMO much better than hard coding the device name into a code change.
That way the user has control which device is sending input into the application.

---
