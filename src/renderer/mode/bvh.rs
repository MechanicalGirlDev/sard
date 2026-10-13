use super::GpuTriangle;

/// Pre-order BVH node. `meta.z` skips the entire subtree, including this node.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct GpuNode {
    pub min: [f32; 4],
    pub max: [f32; 4],
    pub meta: [u32; 4],
}

/// Reorders triangles into contiguous leaf ranges and emits a stackless BVH.
/// The caller bounds triangle counts so triangle and node indices fit in u32.
pub(super) fn build(triangles: &mut [GpuTriangle]) -> Vec<GpuNode> {
    let mut nodes = Vec::new();
    if !triangles.is_empty() {
        append(triangles, 0, &mut nodes);
    }
    nodes
}

fn centroid(triangle: &GpuTriangle, axis: usize) -> f32 {
    // Divide first so finite large coordinates do not overflow their sum.
    triangle.positions.iter().map(|p| p[axis] / 3.0).sum()
}

fn append(triangles: &mut [GpuTriangle], first: usize, nodes: &mut Vec<GpuNode>) {
    let index = nodes.len();
    let mut min = [f32::INFINITY; 4];
    let mut max = [f32::NEG_INFINITY; 4];
    let mut centroid_min = [f32::INFINITY; 3];
    let mut centroid_max = [f32::NEG_INFINITY; 3];
    for triangle in triangles.iter() {
        for axis in 0..3 {
            for position in &triangle.positions {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
            let center = centroid(triangle, axis);
            centroid_min[axis] = centroid_min[axis].min(center);
            centroid_max[axis] = centroid_max[axis].max(center);
        }
    }
    min[3] = 0.0;
    max[3] = 0.0;
    let count = triangles.len();
    nodes.push(GpuNode {
        min,
        max,
        meta: [
            first as u32,
            if count <= 4 { count as u32 } else { 0 },
            0,
            0,
        ],
    });
    if count > 4 {
        let mut axis = 0;
        for candidate in 1..3 {
            if centroid_max[candidate] - centroid_min[candidate]
                > centroid_max[axis] - centroid_min[axis]
            {
                axis = candidate;
            }
        }
        let mid = count / 2;
        triangles
            .select_nth_unstable_by(mid, |a, b| centroid(a, axis).total_cmp(&centroid(b, axis)));
        let (left, right) = triangles.split_at_mut(mid);
        append(left, first, nodes);
        append(right, first + mid, nodes);
    }
    nodes[index].meta[2] = nodes.len() as u32;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle(x: f32) -> GpuTriangle {
        GpuTriangle {
            positions: [
                [x, -1.0, 2.0, 1.0],
                [x + 0.5, 3.0, -2.0, 1.0],
                [x - 0.25, 0.0, 0.0, 1.0],
            ],
            normals: [[0.0, 0.0, 1.0, 0.0]; 3],
            base_color: [x, 0.0, 0.0, 1.0],
            emissive_metallic: [0.0; 4],
            parameters: [0.0; 4],
        }
    }

    #[test]
    fn empty_input_has_no_nodes() {
        assert!(build(&mut []).is_empty());
    }

    #[test]
    fn leaves_cover_each_triangle_and_bounds_contain_vertices() {
        for count in [1, 4, 5, 17, 64, 129] {
            let mut triangles: Vec<_> = (0..count).rev().map(|x| triangle(x as f32)).collect();
            let nodes = build(&mut triangles);
            let mut covered = vec![0; count];
            for node in &nodes {
                if node.meta[1] == 0 {
                    continue;
                }
                assert!(node.meta[1] <= 4);
                let start = node.meta[0] as usize;
                let end = start + node.meta[1] as usize;
                for index in start..end {
                    covered[index] += 1;
                    for p in triangles[index].positions {
                        for (axis, coordinate) in p.iter().enumerate().take(3) {
                            assert!(*coordinate >= node.min[axis]);
                            assert!(*coordinate <= node.max[axis]);
                        }
                    }
                }
            }
            assert!(covered.iter().all(|&coverage| coverage == 1));
            let mut identities: Vec<_> =
                triangles.iter().map(|t| t.base_color[0] as usize).collect();
            identities.sort_unstable();
            assert_eq!(identities, (0..count).collect::<Vec<_>>());
        }
    }

    #[test]
    fn escape_indices_delimit_preorder_subtrees() {
        let mut triangles: Vec<_> = (0..97).map(|x| triangle(x as f32)).collect();
        let nodes = build(&mut triangles);
        assert_eq!(nodes[0].meta[2] as usize, nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            let escape = node.meta[2] as usize;
            assert!(escape > index && escape <= nodes.len());
            if node.meta[1] > 0 {
                assert_eq!(escape, index + 1);
            } else {
                let left = index + 1;
                let right = nodes[left].meta[2] as usize;
                assert!(right > left && right < escape);
                assert_eq!(nodes[right].meta[2] as usize, escape);
                for child in &nodes[index + 1..escape] {
                    for axis in 0..3 {
                        assert!(child.min[axis] >= node.min[axis]);
                        assert!(child.max[axis] <= node.max[axis]);
                    }
                }
            }
        }
    }

    #[test]
    fn coincident_centroids_still_make_bounded_leaves() {
        let mut triangles = vec![triangle(0.0); 33];
        let nodes = build(&mut triangles);
        assert!(nodes.iter().all(|node| node.meta[1] <= 4));
        assert_eq!(nodes.iter().map(|node| node.meta[1]).sum::<u32>(), 33);
    }
}
