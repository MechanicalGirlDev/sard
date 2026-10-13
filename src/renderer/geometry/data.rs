//! CPU-side triangle meshes: generators, OBJ parsing and GPU upload.
//!
//! [`MeshData`] keeps positions / normals / indices on the CPU so a scene can ray-cast
//! against exact triangles (picking) and so the same data can be uploaded to several
//! devices (a headless test, an XR swapchain). [`MeshAsset`] pairs one `MeshData` with
//! its GPU buffers and is shared between objects through [`MeshHandle`].
//! These types are also re-exported by the retained-scene API.

use std::sync::Arc;

use glam::{Mat4, Vec3};

use crate::context::WgpuContext;
use crate::core::buffer::{IndexBuffer, VertexBuffer};
use crate::core::vertex::VertexPN;
use crate::renderer::geometry::Aabb;

/// Shared handle to an uploaded mesh.
pub type MeshHandle = Arc<MeshAsset>;

/// Triangle mesh data on the CPU (indexed, per-vertex normals).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshData {
    /// Vertex positions.
    pub positions: Vec<Vec3>,
    /// Per-vertex unit normals, aligned with `positions`.
    pub normals: Vec<Vec3>,
    /// Triangle list indices into `positions`.
    pub indices: Vec<u32>,
}

/// Errors from [`MeshData::from_obj`].
#[derive(Debug, thiserror::Error)]
pub enum ObjError {
    /// A numeric field could not be parsed.
    #[error("line {line}: bad number `{token}`")]
    BadNumber {
        /// 1-based line number.
        line: usize,
        /// Offending token.
        token: String,
    },
    /// A face references a vertex that does not exist.
    #[error("line {line}: vertex index {index} out of range")]
    IndexOutOfRange {
        /// 1-based line number.
        line: usize,
        /// The (already resolved, 0-based) index.
        index: i64,
    },
    /// A face has fewer than three vertices.
    #[error("line {line}: face needs at least 3 vertices")]
    DegenerateFace {
        /// 1-based line number.
        line: usize,
    },
    /// The file has no triangles at all.
    #[error("no faces")]
    Empty,
}

impl MeshData {
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Axis-aligned bounds of the positions (a degenerate box at the origin when empty).
    pub fn aabb(&self) -> Aabb {
        if self.positions.is_empty() {
            return Aabb::new(Vec3::ZERO, Vec3::ZERO);
        }
        Aabb::from_points(self.positions.iter().copied())
    }

    /// Apply a transform to every position (and its inverse-transpose to the normals).
    pub fn transformed(mut self, m: Mat4) -> Self {
        let n = m.inverse().transpose();
        for p in &mut self.positions {
            *p = m.transform_point3(*p);
        }
        for v in &mut self.normals {
            *v = n.transform_vector3(*v).normalize_or_zero();
        }
        self
    }

    /// Replace the normals with area-weighted face normals accumulated per vertex.
    pub fn recompute_normals(&mut self) {
        let mut acc = vec![Vec3::ZERO; self.positions.len()];
        for tri in self.indices.as_chunks::<3>().0 {
            let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            let n = (self.positions[b] - self.positions[a])
                .cross(self.positions[c] - self.positions[a]);
            acc[a] += n;
            acc[b] += n;
            acc[c] += n;
        }
        self.normals = acc.into_iter().map(Vec3::normalize_or_zero).collect();
    }

    /// Append another mesh (indices are re-based).
    pub fn append(&mut self, other: &Self) {
        let base = self.positions.len() as u32;
        self.positions.extend_from_slice(&other.positions);
        self.normals.extend_from_slice(&other.normals);
        self.indices.extend(other.indices.iter().map(|i| i + base));
    }

    /// Interleave into GPU vertices.
    pub fn vertices(&self) -> Vec<VertexPN> {
        self.positions
            .iter()
            .zip(&self.normals)
            .map(|(p, n)| VertexPN::new(p.to_array(), n.to_array()))
            .collect()
    }

    // ----- generators -----------------------------------------------------------

    /// Axis-aligned box centred at the origin with the given full edge lengths.
    pub fn cuboid(size: Vec3) -> Self {
        let h = size * 0.5;
        let faces: [(Vec3, Vec3, Vec3); 6] = [
            (Vec3::X, Vec3::Y, Vec3::Z),
            (Vec3::NEG_X, Vec3::Z, Vec3::Y),
            (Vec3::Y, Vec3::Z, Vec3::X),
            (Vec3::NEG_Y, Vec3::X, Vec3::Z),
            (Vec3::Z, Vec3::X, Vec3::Y),
            (Vec3::NEG_Z, Vec3::Y, Vec3::X),
        ];
        let mut m = Self::default();
        for (n, u, v) in faces {
            let base = m.positions.len() as u32;
            let c = n * h;
            let du = u * h;
            let dv = v * h;
            m.positions
                .extend([c - du - dv, c + du - dv, c + du + dv, c - du + dv]);
            m.normals.extend([n; 4]);
            m.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        m
    }

    /// UV sphere centred at the origin.
    pub fn sphere(radius: f32, segments: u32, rings: u32) -> Self {
        let segments = segments.max(3);
        let rings = rings.max(2);
        let mut m = Self::default();
        for r in 0..=rings {
            let v = r as f32 / rings as f32;
            let theta = v * core::f32::consts::PI;
            let (st, ct) = theta.sin_cos();
            for s in 0..=segments {
                let u = s as f32 / segments as f32;
                let phi = u * core::f32::consts::TAU;
                let (sp, cp) = phi.sin_cos();
                let n = Vec3::new(st * cp, st * sp, ct);
                m.positions.push(n * radius);
                m.normals.push(n);
            }
        }
        let stride = segments + 1;
        for r in 0..rings {
            for s in 0..segments {
                let a = r * stride + s;
                let b = a + stride;
                m.indices.extend([a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        m
    }

    /// Cylinder along +Z, centred at the origin, with flat caps.
    pub fn cylinder(radius: f32, length: f32, segments: u32) -> Self {
        Self::frustum(radius, radius, length, segments)
    }

    /// Cone along +Z: base disc of `radius` at `z = -length / 2`, apex at `z = +length / 2`.
    pub fn cone(radius: f32, length: f32, segments: u32) -> Self {
        Self::frustum(radius, 0.0, length, segments)
    }

    /// Truncated cone along +Z (`r0` at the bottom, `r1` at the top).
    fn frustum(r0: f32, r1: f32, length: f32, segments: u32) -> Self {
        let segments = segments.max(3);
        let h = length * 0.5;
        let mut m = Self::default();
        // Side: the normal tilts by the slope so cones shade correctly.
        let slope = (r0 - r1) / length;
        let base = m.positions.len() as u32;
        for s in 0..=segments {
            let phi = s as f32 / segments as f32 * core::f32::consts::TAU;
            let (sp, cp) = phi.sin_cos();
            let n = Vec3::new(cp, sp, slope).normalize();
            m.positions.push(Vec3::new(cp * r0, sp * r0, -h));
            m.normals.push(n);
            m.positions.push(Vec3::new(cp * r1, sp * r1, h));
            m.normals.push(n);
        }
        for s in 0..segments {
            let a = base + s * 2;
            m.indices.extend([a, a + 2, a + 1, a + 1, a + 2, a + 3]);
        }
        for (z, r, n) in [(-h, r0, Vec3::NEG_Z), (h, r1, Vec3::Z)] {
            if r <= 0.0 {
                continue;
            }
            let centre = m.positions.len() as u32;
            m.positions.push(Vec3::new(0.0, 0.0, z));
            m.normals.push(n);
            for s in 0..=segments {
                let phi = s as f32 / segments as f32 * core::f32::consts::TAU;
                let (sp, cp) = phi.sin_cos();
                m.positions.push(Vec3::new(cp * r, sp * r, z));
                m.normals.push(n);
            }
            for s in 0..segments {
                let a = centre + 1 + s;
                if n.z > 0.0 {
                    m.indices.extend([centre, a, a + 1]);
                } else {
                    m.indices.extend([centre, a + 1, a]);
                }
            }
        }
        m
    }

    /// Capsule along +Z: a cylinder of `length` between two hemispheres of `radius`.
    pub fn capsule(radius: f32, length: f32, segments: u32, rings: u32) -> Self {
        let h = length * 0.5;
        let mut m = Self::cylinder(radius, length, segments);
        // Drop the flat caps: keep only the side (the first `(segments + 1) * 2` vertices).
        let side = ((segments.max(3) + 1) * 2) as usize;
        m.positions.truncate(side);
        m.normals.truncate(side);
        m.indices.retain(|&i| (i as usize) < side);
        let sphere = Self::sphere(radius, segments, rings.max(2) * 2);
        let mut top = sphere.clone();
        let mut bottom = sphere;
        // Split the sphere at the equator and push each half outwards.
        for p in &mut top.positions {
            p.z = p.z.max(0.0) + h;
        }
        for p in &mut bottom.positions {
            p.z = p.z.min(0.0) - h;
        }
        m.append(&top);
        m.append(&bottom);
        m
    }

    /// Arrow along +Z from the origin to `length`: a shaft of `shaft_diameter` and a
    /// head of `head_diameter` × `head_length` at the tip.
    pub fn arrow(length: f32, shaft_diameter: f32, head_diameter: f32, head_length: f32) -> Self {
        let head_length = head_length.min(length).max(0.0);
        let shaft_length = length - head_length;
        let mut m = Self::default();
        if shaft_length > 0.0 {
            let shaft = Self::cylinder(shaft_diameter * 0.5, shaft_length, 12)
                .transformed(Mat4::from_translation(Vec3::Z * (shaft_length * 0.5)));
            m.append(&shaft);
        }
        if head_length > 0.0 {
            let head = Self::cone(head_diameter * 0.5, head_length, 12).transformed(
                Mat4::from_translation(Vec3::Z * (shaft_length + head_length * 0.5)),
            );
            m.append(&head);
        }
        m
    }

    /// Parse a Wavefront OBJ. Faces with more than three vertices are fan-triangulated;
    /// normals are read from `vn` when every face vertex has one, otherwise recomputed.
    /// Materials, textures and groups are ignored.
    pub fn from_obj(text: &str) -> Result<Self, ObjError> {
        let mut positions: Vec<Vec3> = Vec::new();
        let mut obj_normals: Vec<Vec3> = Vec::new();
        // (position index, normal index) → output vertex, so shared corners are merged.
        let mut vertex_map: std::collections::HashMap<(u32, Option<u32>), u32> =
            std::collections::HashMap::new();
        let mut out = Self::default();
        let mut all_have_normals = true;

        for (line_no, raw) in text.lines().enumerate() {
            let line_no = line_no + 1;
            let line = raw.split('#').next().unwrap_or("").trim();
            let mut it = line.split_whitespace();
            let Some(tag) = it.next() else { continue };
            match tag {
                "v" | "vn" => {
                    let parse = |tok: Option<&str>| -> Result<f32, ObjError> {
                        let tok = tok.unwrap_or("");
                        tok.parse().map_err(|_| ObjError::BadNumber {
                            line: line_no,
                            token: tok.to_owned(),
                        })
                    };
                    let v = Vec3::new(parse(it.next())?, parse(it.next())?, parse(it.next())?);
                    if tag == "v" {
                        positions.push(v);
                    } else {
                        obj_normals.push(v);
                    }
                }
                "f" => {
                    let mut corners: Vec<u32> = Vec::new();
                    for tok in it {
                        let mut parts = tok.split('/');
                        let pi = resolve_index(parts.next(), positions.len(), line_no)?;
                        // Texture index (parts[1]) is skipped.
                        let _ = parts.next();
                        let ni = match parts.next() {
                            Some(s) if !s.is_empty() => {
                                Some(resolve_index(Some(s), obj_normals.len(), line_no)?)
                            }
                            _ => None,
                        };
                        if ni.is_none() {
                            all_have_normals = false;
                        }
                        let key = (pi, ni);
                        let idx = if let Some(&i) = vertex_map.get(&key) {
                            i
                        } else {
                            let i = out.positions.len() as u32;
                            out.positions.push(positions[pi as usize]);
                            out.normals
                                .push(ni.map_or(Vec3::ZERO, |n| obj_normals[n as usize]));
                            let _ = vertex_map.insert(key, i);
                            i
                        };
                        corners.push(idx);
                    }
                    if corners.len() < 3 {
                        return Err(ObjError::DegenerateFace { line: line_no });
                    }
                    for k in 1..corners.len() - 1 {
                        out.indices.extend([corners[0], corners[k], corners[k + 1]]);
                    }
                }
                _ => {}
            }
        }
        if out.indices.is_empty() {
            return Err(ObjError::Empty);
        }
        if !all_have_normals {
            out.recompute_normals();
        }
        Ok(out)
    }
}

/// Resolve a 1-based (or negative, relative) OBJ index to 0-based.
fn resolve_index(tok: Option<&str>, len: usize, line: usize) -> Result<u32, ObjError> {
    let tok = tok.unwrap_or("");
    let raw: i64 = tok.parse().map_err(|_| ObjError::BadNumber {
        line,
        token: tok.to_owned(),
    })?;
    let idx = if raw < 0 { len as i64 + raw } else { raw - 1 };
    if idx < 0 || idx >= len as i64 {
        return Err(ObjError::IndexOutOfRange { line, index: idx });
    }
    Ok(idx as u32)
}

/// A mesh uploaded to one device, with its CPU data retained for picking.
pub struct MeshAsset {
    data: MeshData,
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer,
    aabb: Aabb,
}

impl MeshAsset {
    /// Upload `data`.
    pub fn new(ctx: &WgpuContext, data: MeshData, label: Option<&str>) -> Self {
        let vertex_buffer = VertexBuffer::new(ctx, &data.vertices(), label);
        let index_buffer = IndexBuffer::new_u32(ctx, &data.indices, label);
        let aabb = data.aabb();
        Self {
            data,
            vertex_buffer,
            index_buffer,
            aabb,
        }
    }

    /// Upload and wrap in a shared handle.
    pub fn shared(ctx: &WgpuContext, data: MeshData, label: Option<&str>) -> MeshHandle {
        Arc::new(Self::new(ctx, data, label))
    }

    /// The CPU data.
    pub fn data(&self) -> &MeshData {
        &self.data
    }

    /// Local-space bounds.
    pub fn aabb(&self) -> Aabb {
        self.aabb
    }

    pub(crate) fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vertex_buffer
    }

    pub(crate) fn index_buffer(&self) -> &IndexBuffer {
        &self.index_buffer
    }
}

impl core::fmt::Debug for MeshAsset {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MeshAsset")
            .field("triangles", &self.data.triangle_count())
            .field("aabb", &self.aabb)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit_normals(m: &MeshData) {
        assert_eq!(m.positions.len(), m.normals.len());
        for n in &m.normals {
            assert!((n.length() - 1.0).abs() < 1e-4, "{n:?}");
        }
        assert_eq!(m.indices.len() % 3, 0);
        for &i in &m.indices {
            assert!((i as usize) < m.positions.len());
        }
    }

    #[test]
    fn generators_are_well_formed() {
        for m in [
            MeshData::cuboid(Vec3::new(1.0, 2.0, 3.0)),
            MeshData::sphere(0.5, 8, 6),
            MeshData::cylinder(0.2, 1.0, 8),
            MeshData::cone(0.2, 1.0, 8),
            MeshData::capsule(0.2, 1.0, 8, 4),
            MeshData::arrow(1.0, 0.05, 0.1, 0.2),
        ] {
            unit_normals(&m);
            assert!(m.triangle_count() > 0);
        }
        let b = MeshData::cuboid(Vec3::new(1.0, 2.0, 3.0)).aabb();
        assert_eq!(b.min, Vec3::new(-0.5, -1.0, -1.5));
        assert_eq!(b.max, Vec3::new(0.5, 1.0, 1.5));
        // The capsule spans the cylinder plus both hemispheres.
        let c = MeshData::capsule(0.2, 1.0, 8, 4).aabb();
        assert!((c.max.z - 0.7).abs() < 1e-5 && (c.min.z + 0.7).abs() < 1e-5);
        // Arrow tip sits at `length`.
        let a = MeshData::arrow(1.0, 0.05, 0.1, 0.2).aabb();
        assert!((a.max.z - 1.0).abs() < 1e-5 && a.min.z.abs() < 1e-5);
    }

    #[test]
    fn obj_quads_negative_indices_and_missing_normals() {
        let obj = "# a quad\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\nf -4 -3 -2\n";
        let m = MeshData::from_obj(obj).unwrap();
        assert_eq!(m.positions.len(), 4);
        assert_eq!(m.triangle_count(), 3);
        unit_normals(&m);
        assert!((m.normals[0] - Vec3::Z).length() < 1e-5);
    }

    #[test]
    fn obj_keeps_supplied_normals_and_splits_by_normal() {
        let obj =
            "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nvn 1 0 0\nf 1//1 2//1 3//1\nf 1//2 2//2 3//2\n";
        let m = MeshData::from_obj(obj).unwrap();
        // Same positions, two normals → six output vertices.
        assert_eq!(m.positions.len(), 6);
        assert!((m.normals[0] - Vec3::Z).length() < 1e-5);
        assert!((m.normals[3] - Vec3::X).length() < 1e-5);
    }

    #[test]
    fn obj_errors() {
        assert!(matches!(
            MeshData::from_obj("v 0 0 x\n"),
            Err(ObjError::BadNumber { line: 1, .. })
        ));
        assert!(matches!(
            MeshData::from_obj("v 0 0 0\nf 1 2 3\n"),
            Err(ObjError::IndexOutOfRange { line: 2, .. })
        ));
        assert!(matches!(
            MeshData::from_obj("v 0 0 0\nv 1 0 0\nf 1 2\n"),
            Err(ObjError::DegenerateFace { line: 3 })
        ));
        assert!(matches!(
            MeshData::from_obj("v 0 0 0\n"),
            Err(ObjError::Empty)
        ));
    }

    #[test]
    fn transformed_moves_positions_and_rotates_normals() {
        let m = MeshData::cuboid(Vec3::ONE)
            .transformed(Mat4::from_rotation_x(core::f32::consts::FRAC_PI_2));
        unit_normals(&m);
        // +Y face normal rotates to +Z.
        let has_z = m.normals.iter().any(|n| (*n - Vec3::Z).length() < 1e-5);
        assert!(has_z);
    }
}
