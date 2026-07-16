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

## Traversal performance

Hyperbolic nearby-cell counts grow exponentially with distance. Recomputing a breadth-first graph
walk every frame is therefore forbidden in hot rendering or generation paths. `NearbyCache`
performs a padded traversal and reuses it until movement, cell transitions, graph growth, or a
distance change invalidates the result. Render preparation applies an exact distance test to the
conservative cached set, while world generation stops scanning once all eligible chunks have been
scheduled.

Baseline at 2560x1440 and 85 m on the development machine:

- Nearby traversal median before caching: 8.39 ms per frame.
- Nearby traversal median after caching: below 0.001 ms on cache-hit frames.
- Draw-thread CPU median before the pass: 10.23 ms.
- Draw-thread CPU median after the pass: 1.91 ms.
- Settled world-generation scan median after the pass: below 0.002 ms.

These numbers are diagnostic baselines, not universal hardware promises. Re-run the same profile
after changes to traversal, chunk scheduling, render distance, surface extraction, or LOD logic.
