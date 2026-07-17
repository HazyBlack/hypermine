use crate::{
    graph::Graph,
    node::ChunkId,
    voxel_math::{ChunkDirection, CoordAxis, CoordSign, Coords},
};

/// A voxel plus a tool-local orthogonal frame that remains coherent while crossing chunk frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelCursor {
    pub chunk: ChunkId,
    pub coords: Coords,
    axes: [ChunkDirection; 3],
}

impl VoxelCursor {
    /// Creates a frame whose local Z axis points into the face that was hit. Local X and Y provide
    /// the pick's width and height directions respectively.
    pub fn from_hit(
        chunk: ChunkId,
        coords: Coords,
        face_axis: CoordAxis,
        face_sign: CoordSign,
    ) -> Self {
        let [width_axis, height_axis] = face_axis.other_axes();
        Self {
            chunk,
            coords,
            axes: [
                ChunkDirection {
                    axis: width_axis,
                    sign: CoordSign::Plus,
                },
                ChunkDirection {
                    axis: height_axis,
                    sign: CoordSign::Plus,
                },
                ChunkDirection {
                    axis: face_axis,
                    sign: opposite(face_sign),
                },
            ],
        }
    }

    pub fn step(self, graph: &Graph, local_axis: usize, sign: CoordSign) -> Option<Self> {
        let direction = self.local_direction(local_axis, sign);
        let crosses_boundary = self.coords[direction.axis]
            == Coords::boundary_coord(graph.layout().dimension(), direction.sign);
        let permutation = (crosses_boundary && direction.sign == CoordSign::Plus)
            .then(|| self.chunk.vertex.chunk_axis_permutations()[direction.axis as usize]);
        let (chunk, coords) =
            graph.get_block_neighbor(self.chunk, self.coords, direction.axis, direction.sign)?;
        let axes = if crosses_boundary {
            let reflected_axis = permutation.map_or(direction.axis, |p| p[direction.axis]);
            self.axes.map(|axis| {
                let mut axis = permutation.map_or(axis, |p| p * axis);
                if axis.axis == reflected_axis {
                    axis.sign = opposite(axis.sign);
                }
                axis
            })
        } else {
            self.axes
        };
        Some(Self {
            chunk,
            coords,
            axes,
        })
    }

    pub fn local_direction(self, local_axis: usize, sign: CoordSign) -> ChunkDirection {
        signed_direction(self.axes[local_axis], sign)
    }

    /// Server-side variant that creates a missing dodecahedral neighbor when crossing a negative
    /// chunk face, allowing long admin jobs to proceed beyond the currently generated terrain.
    pub fn step_ensuring(self, graph: &mut Graph, local_axis: usize, sign: CoordSign) -> Self {
        let direction = self.local_direction(local_axis, sign);
        if direction.sign == CoordSign::Minus
            && self.coords[direction.axis]
                == Coords::boundary_coord(graph.layout().dimension(), direction.sign)
        {
            let side = self.chunk.vertex.canonical_sides()[direction.axis as usize];
            graph.ensure_neighbor(self.chunk.node, side);
        }
        self.step(graph, local_axis, sign)
            .expect("ensured voxel neighbor exists")
    }
}

fn signed_direction(mut direction: ChunkDirection, sign: CoordSign) -> ChunkDirection {
    if sign == CoordSign::Minus {
        direction.sign = opposite(direction.sign);
    }
    direction
}

fn opposite(sign: CoordSign) -> CoordSign {
    match sign {
        CoordSign::Plus => CoordSign::Minus,
        CoordSign::Minus => CoordSign::Plus,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dodeca::Vertex, graph::NodeId};

    #[test]
    fn tool_frame_keeps_moving_and_round_trips_across_boundaries() {
        let mut graph = Graph::new(3);
        let start = VoxelCursor::from_hit(
            ChunkId::new(NodeId::ROOT, Vertex::A),
            Coords([1, 1, 1]),
            CoordAxis::Z,
            CoordSign::Plus,
        );
        for axis in 0..3 {
            for sign in [CoordSign::Plus, CoordSign::Minus] {
                let mut cursor = start;
                let mut visited = std::collections::HashSet::new();
                visited.insert((cursor.chunk, cursor.coords.0));
                for _ in 0..20 {
                    cursor = cursor.step_ensuring(&mut graph, axis, sign);
                    assert!(
                        visited.insert((cursor.chunk, cursor.coords.0)),
                        "straight traversal bounced onto an already visited voxel"
                    );
                }
                for _ in 0..20 {
                    cursor = cursor.step_ensuring(&mut graph, axis, opposite(sign));
                }
                assert_eq!(cursor, start);
            }
        }
    }
}
