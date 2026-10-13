# Full FAWE compatibility — remaining work

Target: [FastAsyncWorldEdit](https://modrinth.com/plugin/fastasyncworldedit), while retaining Pumpkin. The installed native command subset does not meet the requested full compatibility scope.

## Full compatibility requirements

Use the actual EngineHub WorldEdit engine through a Pumpkin platform adapter, or port that engine and its behavior comprehensively. The small native command implementation must not be presented as equivalent to that engine.

Examined Pumpkin revision: `204a94ed895f041a845630d5a23c094705a0704e`, plugin interface `v0.1`.

Concrete gaps in that interface:

- `World` and `Chunk` expose biome reads, but no live biome setter. The `ChunkBuffer` biome setter is a world-generation interface, not an existing-world editor.
- Generic entities lack a complete NBT export/import interface required for lossless entity clipboard/schematic operations.
- The exposed chunk lookup returns loaded chunks; a complete editor needs controlled chunk loading and generation with bounded work, plus region regeneration support.

An engine adapter also needs matching block/item/biome/entity registries, lossless block-entity NBT, player and actor integration, all commands and permissions, wand/brush interaction events, scheduling, schematic storage and data-version conversion. Undo must restore block states, block-entity contents, biomes and entities, not just the simple block states supported by this prototype.

Before deployment, compare command and operation results against official WorldEdit on a disposable fixture world, including selections, masks, patterns, transforms, brushes, schematic round trips, containers, entities, biomes, regeneration, undo/redo, cancellation, non-operator denial, and CPU/memory limits. A full port must retain existing Pumpkin plugins and the live world's data.

References:

- https://worldedit.enginehub.org/en/latest/api/
- https://worldedit.enginehub.org/en/latest/commands/
- https://github.com/EngineHub/WorldEdit/tree/version/7.4.x

The prototype is independently written Rust; it is not an official EngineHub product or a translation of the complete WorldEdit source. Incorporating upstream code requires preserving its GPL licensing and notices.
