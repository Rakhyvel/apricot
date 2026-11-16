//! Splitting endless terrain into square chunks you build as you get near them

use nalgebra_glm::Vec3;

use crate::mesh::MeshData;

/// Which chunk, counted in chunks from the origin along X and Z
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub z: i32,
}

impl ChunkCoord {
    pub fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

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
        ChunkCoord::new((pos.x / size).floor() as i32, (pos.z / size).floor() as i32)
    }

    /// Where a chunk's corner with the smallest X and Z sits, at height 0
    pub fn origin(&self, chunk: ChunkCoord) -> Vec3 {
        let size = self.chunk_size();
        nalgebra_glm::vec3(chunk.x as f32 * size, 0.0, chunk.z as f32 * size)
    }

    /// Every chunk within `radius` chunks of `pos`, closest first.
    pub fn chunks_near(&self, pos: Vec3, radius: i32) -> Vec<ChunkCoord> {
        let here = self.chunk_at(pos);
        let mut chunks: Vec<(i32, ChunkCoord)> = (-radius..=radius)
            .flat_map(|dx| (-radius..=radius).map(move |dz| (dx, dz)))
            .map(|(dx, dz)| (dx * dx + dz * dz, ChunkCoord::new(here.x + dx, here.z + dz)))
            .filter(|(dist_sq, _)| *dist_sq <= radius * radius)
            .collect();
        chunks.sort_by_key(|(dist_sq, _)| *dist_sq);
        chunks.into_iter().map(|(_, chunk)| chunk).collect()
    }

    /// The terrain mesh for a chunk, with `height(x, z)` sampled at world positions. Its
    /// positions are relative to the chunk's corner
    pub fn mesh(&self, chunk: ChunkCoord, height: impl Fn(f32, f32) -> f32) -> MeshData {
        let origin = self.origin(chunk);
        super::mesh(self.chunk_cells, self.chunk_cells, |x, z| {
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
        assert_eq!(grid.chunk_at(vec3(0.0, 5.0, 15.9)), ChunkCoord::new(0, 0));
        assert_eq!(grid.chunk_at(vec3(-0.1, 0.0, 16.0)), ChunkCoord::new(-1, 1));
        assert_eq!(grid.origin(ChunkCoord::new(-1, 1)), vec3(-16.0, 0.0, 16.0));
    }

    #[test]
    fn chunks_near_starts_with_the_one_underfoot() {
        let grid = ChunkGrid::new(16);
        let near = grid.chunks_near(vec3(40.0, 0.0, -8.0), 2);
        assert_eq!(near[0], ChunkCoord::new(2, -1));
        assert_eq!(near.len(), 13); // a circle of radius 2 on the grid
    }

    #[test]
    fn chunks_near_is_every_chunk_inside_the_circle_closest_first() {
        let grid = ChunkGrid::new(16);
        let near = grid.chunks_near(vec3(-40.0, 0.0, 70.0), 3);
        let (cx, cz) = (-3, 4);
        assert_eq!(near[0], ChunkCoord::new(cx, cz));
        assert_eq!(near.len(), 29); // grid points with dx^2 + dz^2 <= 9
        assert!(
            near.contains(&ChunkCoord::new(cx + 3, cz))
                && near.contains(&ChunkCoord::new(cx - 2, cz + 2))
        );
        assert!(
            !near.contains(&ChunkCoord::new(cx + 2, cz + 3)),
            "2^2 + 3^2 is past the edge"
        );
        let dist_sq = |c: &ChunkCoord| (c.x - cx).pow(2) + (c.z - cz).pow(2);
        assert!(near.windows(2).all(|w| dist_sq(&w[0]) <= dist_sq(&w[1])));
    }

    /// (position across the edge, height) of every vertex where local `axis` is `at`, rounded
    /// so they compare cleanly
    fn edge(mesh: &MeshData, axis: usize, at: f32) -> Vec<(i32, i32)> {
        let across = 2 - axis; // x edges run along z, z edges along x
        let mut points: Vec<(i32, i32)> = mesh
            .vertices
            .iter()
            .map(|v| v.position)
            .filter(|p| p[axis] == at)
            .map(|p| (p[across] as i32, (p[1] * 1000.0).round() as i32))
            .collect();
        points.sort();
        points.dedup();
        points
    }

    #[test]
    fn neighboring_chunks_share_their_edge() {
        let grid = ChunkGrid::new(4);
        let height = |x: f32, z: f32| (x * 0.3).sin() + (z * 0.2).cos();
        // Away from the origin, so mixing up adding and multiplying the offset shows
        let chunk = grid.mesh(ChunkCoord::new(2, -3), height);
        let along_x = grid.mesh(ChunkCoord::new(3, -3), height);
        let along_z = grid.mesh(ChunkCoord::new(2, -2), height);
        let x_edge = edge(&chunk, 0, 4.0);
        let z_edge = edge(&chunk, 2, 4.0);
        assert_eq!(x_edge.len(), 5);
        assert_eq!(z_edge.len(), 5);
        assert_eq!(x_edge, edge(&along_x, 0, 0.0));
        assert_eq!(z_edge, edge(&along_z, 2, 0.0));
    }
}
