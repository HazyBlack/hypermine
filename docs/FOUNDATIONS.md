# HyperMine Content and Performance Foundations

## Compatibility contract

- Material IDs `0..=39` are permanent. Existing IDs must never be reordered or reused.
- New materials are appended and described in `Material::DEFINITIONS` with a unique key,
  player-facing name, exact texture filename, and broad material class.
- `save::CURRENT_FORMAT_VERSION`, `CURRENT_CONTENT_REGISTRY_VERSION`, and
  `CURRENT_WORLDGEN_VERSION` are incremented intentionally when their corresponding contracts
  change.
- Legacy version-zero saves are backed up before being upgraded to the current metadata baseline.
- Network peers must agree on `proto::PROTOCOL_VERSION` before simulation begins.

## Large block operations

Geometry tools should construct a `BlockEditBatch` of inventory-independent `VoxelEdit` values.
The authoritative gameplay layer remains responsible for validating permissions, range, material
costs, drops, and undo data. A batch is rejected when it exceeds 65,536 edits. Implementations
should group dirty chunks, network deltas, mesh invalidation, drops, and undo history around the
logical batch instead of treating every voxel as a separate player action.

The creative-only Admin Pick is item ID 40; it is deliberately not a `Material`, so it can never
be placed as a block or serialized into voxel terrain. Its tool-local width, height, and depth axes
are transported through chunk-axis permutations by `VoxelCursor`, keeping the selected volume
coherent across the hyperbolic tiling.

Admin digs are compact jobs rather than preallocated edit lists. Each axis is bounded to 1,000,
including the one-billion-block maximum volume, and jobs visit the aimed block first before
expanding outward. The server applies at most 1,024 candidate edits per player per simulation step,
reports remaining work to the client, and supports cancellation. This prevents a large request from
allocating a billion records or monopolizing one frame, though enormous jobs can still take a long
time and create correspondingly large saves.

Live Admin Pick results are transmitted as `ChunkVoxelEdits`: one chunk address followed by compact
coordinate/material pairs. Both server and client invalidate a touched chunk once per group instead
of once per voxel. The renderer integrates at most 4,096 bulk edits per frame, network intake is
bounded per frame, and the client reports queued plus not-yet-loaded edits to the server. A dig
automatically pauses at a 16,384-edit client backlog and resumes as terrain catches up. This keeps
the authoritative save progressing without allowing the display queue to grow without bound.

## World registry and deletion

The version-2 world registry stores independent display names, saves, compatibility versions, and
pending managed deletions. World configuration may rename a world and change its per-world
creative/survival inventory mode without regenerating terrain. Deletion requires a confirmation
screen, refuses to remove the final world, and only removes relative paths inside Hypermine's data
directory. A loaded Windows save that is still locked is recorded for retry on the next launch.

## Transient geometry previews

Selection, block-damage cracks, geodesic construction guides, region boundaries, and drill
footprints should use the lightweight transient-geometry render path rather than creating world
blocks or persistent chunk meshes. A voxel raycast carries its normalized chunk-to-view transform
with the hit, so previews never need to search the exponentially large nearby-cell traversal.

`Selection` is the first implementation: it generates a highlighted cube directly in the vertex
shader with one small draw call and no vertex-buffer allocation. New geometry tools should extend
this shared path with bounded preview descriptions; preview rendering must remain visual-only, while
committed edits continue through the authoritative `BlockEditBatch` path.

## Traversal performance

Hyperbolic nearby-cell counts grow exponentially with distance. Recomputing a breadth-first graph
walk every frame is therefore forbidden in hot rendering or generation paths. `NearbyCache`
performs a padded traversal and reuses it until movement, cell transitions, graph growth, or a
distance change invalidates the result. Render preparation applies an exact distance test to the
conservative cached set, while world generation stops scanning once all eligible chunks have been
scheduled.

Cache padding must never expand the generated graph beyond the configured generation radius. The
renderer reuses the generation traversal because it cannot draw terrain that has not been generated.
Generation refreshes after roughly one metre of movement, while completed chunks and new surface
meshes are each admitted at a maximum of 32 per frame. These budgets trade a little streaming latency
for much lower frame-time spikes.

Cell transitions rebase the cached traversal through a single shared basis transform. Never expose
cached node transforms relative to the previous reference cell, and never rebuild the full traversal
solely because the player's canonical cell changed.

Baseline at 2560x1440 and 85 m on the development machine:

- Nearby traversal median before caching: 8.39 ms per frame.
- Nearby traversal median after caching: below 0.001 ms on cache-hit frames.
- Draw-thread CPU median before the pass: 10.23 ms.
- Draw-thread CPU median after the pass: 1.91 ms.
- Settled world-generation scan median after the pass: below 0.002 ms.

These numbers are diagnostic baselines, not universal hardware promises. Re-run the same profile
after changes to traversal, chunk scheduling, render distance, surface extraction, or LOD logic.
