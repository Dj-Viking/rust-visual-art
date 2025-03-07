# rust-visual-art

notes for myself - this readme is a WIP

## hot-reloading
To set up the hot-reloading - run this command in a separate shell
to watch the changes to lib/*.rs which are the "plugin" functions 
that control the visuals

NOTE: requires cargo watch - install with `cargo install cargo-watch`
```sh
cargo watch -w lib -x 'r --manifest-path build_script/Cargo.toml'
# or
sh watch.sh
```

## midi

TODO


## audio

TODO
