# Rust-WorldEdit

Native **WorldEdit Core** for Pumpkin, with operator-only selection and basic editing. This independently written Rust/WASM implementation is **not the complete FastAsyncWorldEdit engine**. Full FAWE compatibility remains unfinished; see [PORTING.md](PORTING.md).

## Use

Run `//wand`, then equip the wooden axe from your inventory. Left-click a block for position 1 and right-click another for position 2. Wand clicks select without breaking blocks.

| Command | Behavior |
| --- | --- |
| `//wand` | Give a wooden selection axe |
| `//pos1 [X,Y,Z]`, `//pos2 [X,Y,Z]` | Select exact coordinates or your current block |
| `//size`, `//desel` | Show selection size or clear it |
| `//set stone` | Fill the selection |
| `//replace stone dirt` | Replace matching blocks |
| `//copy` | Copy blocks relative to your position |
| `//paste [-a]` | Paste relative to your new position; `-a` skips air |
| `//undo`, `//redo` | Undo or redo one edit |
| `//cancel` | Stop the current job; completed changes remain undoable |
| `/wehelp` | Show help and limits |

Block states such as `oak_log[axis=y]` are supported. The pinned Pumpkin host strips an extra slash from modern command packets, so plain-name aliases are registered too. Vanilla `/help` is preserved.

## Limits and data protection

- Actual operator status is checked in command, wand, and edit handlers.
- Maximum 4,096 loaded blocks per operation, 64 changes per tick, one edit running at a time.
- Eight undo levels per player; selections, clipboards, and history are memory-only and clear on plugin unload or server restart.
- Containers, signs, and other blocks with stored data are rejected to avoid lossy copy/undo.
- Edits stop if the operator disconnects, loses operator status, a chunk unloads, or a target changes unexpectedly.
- No brushes, schematic files, entities, biomes, regeneration, FAWE expression/pattern language, or full FAWE compatibility yet.

## Build and install

Clone `https://github.com/kyurs/Rust-WorldEdit.git` and run `rustup target add wasm32-wasip2`. Run `./build.sh` from the checkout; it runs unit tests, WASM Clippy, and a release build. Output: `dist/worldedit.wasm`.

Pinned Pumpkin SDK: `204a94ed895f041a845630d5a23c094705a0704e`.

Copy the WASM into Pumpkin's `plugins/` directory and add:

```toml
[plugins.overrides.WorldEditCore]
enabled = true
allow_unsigned = true
max_memory_mb = 64
allowed_permissions = []
```

Restart Pumpkin after installation or an update. The pinned host does not remove registered permissions when unloading plugins, so hot reloading this plugin fails on duplicate permission registration.

## Validation

Prior integration coverage: wand delivery and both clicks without block destruction; set, replace, copy/paste, undo/redo; non-operator wand denial; oversized-edit rejection; and refusal to overwrite a chest. Four unit tests, WASM Clippy, and the release build passed. Bedrock/Eaglercraft client UI behavior was not manually checked.

`tests/live-probe.cjs` is a Java 1.12.2 protocol integration test for an explicitly authorized server. It requires temporary operator access for `WorldEditAdminQA` and a non-operator account `WEGuestQA`; revoke test permissions afterward. It checks initially empty fixture locations and removes its fixtures in `finally`.

```sh
WORLDEDIT_TEST_HOST=your-server NODE_PATH=/path/to/node_modules node tests/live-probe.cjs
```

The original WorldEdit and FAWE projects are not affiliated with this implementation. No upstream Java source is bundled.

## Related KYURS plugins

These projects are independently installed; none requires the other three.

- [Custom Skins](https://github.com/kyurs/Rust-CustomSkins): official skins and moderation on Velocity.
- [Pumpkin Voice](https://github.com/kyurs/Rust-PumpkinVoice): native Pumpkin proximity voice.
- [Rust Tab List](https://github.com/kyurs/Rust-TabList): native ranks, chat and metrics with a Velocity bridge.
- [Rust WorldEdit](https://github.com/kyurs/Rust-WorldEdit): native operator-only core editing.

License: MIT; see [LICENSE](LICENSE). Release binaries are available under Releases. Back up your world before editing or upgrading. To uninstall, stop Pumpkin, remove `plugins/worldedit.wasm`, and restart.
