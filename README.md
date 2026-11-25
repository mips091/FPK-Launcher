# FPK Launcher

A Minecraft Java Edition launcher prototype built with Rust, targeting **Minecraft Java 26.3**.

## Current prototype
- Fetches Mojang's official version manifest.
- Lists release and snapshot versions.
- Downloads version metadata and the vanilla client JAR.
- Checks the downloaded JAR size and SHA-1 against Mojang's metadata before saving it.
- Downloads and verifies the asset index and asset objects.
- Installs library artifacts and Linux native classifiers declared in the version metadata.
- Safely extracts Linux native libraries into the version's native directory.
- Adds an initial Java process launcher using Mojang's JVM/game arguments and an offline profile.
- Revalidates cached assets and libraries before reusing them.
- Defaults the install command to Minecraft Java 26.3.
- Uses the platform data directory; set `FPK_HOME` to override it.

## Build
```sh
cargo build --release
cargo build --release -p fpk-gui
```

Run the desktop GUI with:
```sh
cargo run --release -p fpk-gui
```

The GUI currently provides version selection, install/repair, and an offline-profile Play action.

## CLI
```sh
cargo run -p fpk-cli -- home
cargo run -p fpk-cli -- list release
cargo run -p fpk-cli -- list snapshot
cargo run -p fpk-cli -- install
# Or choose another version explicitly:
cargo run -p fpk-cli -- install 26.3
# Launch the installed version (offline profile for local testing):
cargo run -p fpk-cli -- launch --username Player
```

**Limitations:** Microsoft account authentication Microsoft account authentication is not implemented yet. The current `launch` command uses an offline profile for local testing; online multiplayer authentication is not provided.
