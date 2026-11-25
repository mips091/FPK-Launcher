# FPK Launcher

A Minecraft Java launcher prototype built with Rust.

## Current prototype
- Fetches Mojang's official version manifest.
- Lists release and snapshot versions.
- Downloads version metadata and the vanilla client JAR.
- Uses the platform data directory; set `FPK_HOME` to override it.

## Build
```sh
cargo build --release
```

## CLI
```sh
cargo run -p fpk-cli -- home
cargo run -p fpk-cli -- list release
cargo run -p fpk-cli -- list snapshot
cargo run -p fpk-cli -- install 1.21.11
```

This is an early bootstrap, not yet a complete game launcher. Next work: library/assets/native resolution, Java selection, authentication, and launching the game process.