# rust-visual-art

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

For now, in order to build this project; 
you'll need to install the portmidi library through your package manager.

I built this on arch linux so it would be like so:

```sh
sudo pacman --sync portmidi
```

However if you don't wish to use a midi controller at the moment 
and just use the keyboard


### audio

I personally am using pulseaudio on my system. So installing
pulseaudio is a must.

```sh
sudo pacman -S pulseaudio
```

If you want to use your own audio device,
(I don't have a better way to do this at the moment)
you'll need to change this line in the audio.rs file

https://github.com/Dj-Viking/rust-visual-art/blob/a133cc338d5bf954bd7d1e9ae24d5b614c719db6/src/audio.rs#L49

find the name of your device that you want to use when found through
running this command in your terminal
and choose your __input__ device
```sh
pacmd list-sources | grep 'name:'
```

Also until I fix this, the latency is pretty bad unless I have `pavucontrol`
running which will demand lower latency from the server, and then
any other client actually benefits from this latency demand. I still dont
fully understand why this happens yet or how I can fix it so I don't need `pavucontrol` running.

---
