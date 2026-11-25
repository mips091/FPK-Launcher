# FPK Launcher

A Minecraft Java Edition launcher prototype built with Rust, targeting **Minecraft Java 26.3**.

## Current prototype
- Fetches Mojang's official version manifest.
- Lists release and snapshot versions.
- Downloads version metadata and the vanilla client JAR.
- Checks the downloaded JAR size and SHA-1 against Mojang's metadata before saving it.
- Downloads and verifies the asset index and asset objects.
- Installs library artifacts declared in the version metadata.
- Revalidates cached assets and libraries before reusing them.
- Defaults the install command to Minecraft Java 26.3.
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
cargo run -p fpk-cli -- install
# Or choose another version explicitly:
cargo run -p fpk-cli -- install 26.3
```

This is an early bootstrap, not yet a complete game launcher. Next work: library/assets/native resolution, Java selection, authentication, and launching the game process.
