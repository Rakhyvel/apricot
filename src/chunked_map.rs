//! Splitting endless terrain into square chunks you build as you get near them

use nalgebra_glm::Vec3;

use crate::mesh_data::MeshData;

/// Which chunk, counted in chunks from the origin along X and Z
pub type ChunkCoord = (i32, i32);

/// Splits the XZ plane into square chunks of `chunk_cells` by `chunk_cells` cells, each cell 1
/// unit across. Chunks are stored somewhere else (e.g. a `ModelSet<ChunkCoord>`)
#[derive(Debug, Clone, Copy)]
pub struct ChunkGrid {
    pub chunk_cells: u32,
}

impl ChunkGrid {
    /// A grid of chunks `chunk_cells` cells across
    pub fn new(chunk_cells: u32) -> Self {
        Self { chunk_cells }
    }

    /// How wide a chunk is, in world units
    pub fn chunk_size(&self) -> f32 {
        self.chunk_cells as f32
    }

    /// The chunk `pos` is over
    pub fn chunk_at(&self, pos: Vec3) -> ChunkCoord {
        let size = self.chunk_size();
        ((pos.x / size).floor() as i32, (pos.z / size).floor() as i32)
    }

    /// Where a chunk's corner with the smallest X and Z sits, at height 0. Put the chunk's model
    /// here
    pub fn origin(&self, chunk: ChunkCoord) -> Vec3 {
        let size = self.chunk_size();
        nalgebra_glm::vec3(chunk.0 as f32 * size, 0.0, chunk.1 as f32 * size)
    }

    /// Every chunk within `radius` chunks of `pos`, closest first.
    pub fn chunks_near(&self, pos: Vec3, radius: i32) -> Vec<ChunkCoord> {
        let (cx, cz) = self.chunk_at(pos);
        let mut chunks: Vec<(i32, ChunkCoord)> = (-radius..=radius)
            .flat_map(|dx| (-radius..=radius).map(move |dz| (dx, dz)))
            .map(|(dx, dz)| (dx * dx + dz * dz, (cx + dx, cz + dz)))
            .filter(|(dist_sq, _)| *dist_sq <= radius * radius)
            .collect();
        chunks.sort_by_key(|(dist_sq, _)| *dist_sq);
        chunks.into_iter().map(|(_, chunk)| chunk).collect()
    }

    /// The terrain mesh for a chunk, with `height(x, z)` sampled at world positions so neighbors
    /// line up. Its positions start at 0, so place it at `origin(chunk)`
    pub fn mesh(&self, chunk: ChunkCoord, height: impl Fn(f32, f32) -> f32) -> MeshData {
        let origin = self.origin(chunk);
        MeshData::heightmap(self.chunk_cells, self.chunk_cells, |x, z| {
            height(origin.x + x, origin.z + z)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec3;

    #[test]
    fn chunk_at_rounds_down_past_zero() {
        let grid = ChunkGrid::new(16);
        assert_eq!(grid.chunk_at(vec3(0.0, 5.0, 15.9)), (0, 0));
        assert_eq!(grid.chunk_at(vec3(-0.1, 0.0, 16.0)), (-1, 1));
        assert_eq!(grid.origin((-1, 1)), vec3(-16.0, 0.0, 16.0));
    }

    #[test]
    fn chunks_near_starts_with_the_one_underfoot() {
        let grid = ChunkGrid::new(16);
        let near = grid.chunks_near(vec3(40.0, 0.0, -8.0), 2);
        assert_eq!(near[0], (2, -1));
        assert_eq!(near.len(), 13); // a circle of radius 2 on the grid
    }

    #[test]
    fn neighboring_chunks_share_their_edge() {
        let grid = ChunkGrid::new(4);
        let height = |x: f32, z: f32| (x * 0.3).sin() + (z * 0.2).cos();
        let (left, right) = (grid.mesh((0, 0), height), grid.mesh((1, 0), height));
        // (z, height) of every vertex along an edge, rounded so they compare cleanly
        let edge = |mesh: &MeshData, local_x: f32| -> Vec<(i32, i32)> {
            let mut points: Vec<(i32, i32)> = mesh
                .positions
                .chunks(3)
                .filter(|p| p[0] == local_x)
                .map(|p| (p[2] as i32, (p[1] * 1000.0).round() as i32))
                .collect();
            points.sort();
            points.dedup();
            points
        };
        assert_eq!(edge(&left, 4.0), edge(&right, 0.0));
    }
}
