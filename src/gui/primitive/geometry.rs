use super::{PrimitiveRenderer, Vertex};

impl PrimitiveRenderer {
    /// Add a rectangle to the draw list.
    pub fn draw_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.push_quad(x, y, w, h, [0.0, 0.0], [1.0, 1.0], color, 0);
    }

    /// Reserve a rectangle whose geometry is filled in later with
    /// [`Self::set_reserved_rect`] — a panel background drawn *behind* content whose
    /// size is only known after the content is laid out.
    pub fn reserve_rect(&mut self) -> usize {
        let index = self.vertices.len();
        self.push_quad(0.0, 0.0, 0.0, 0.0, [0.0, 0.0], [1.0, 1.0], [0.0; 4], 0);
        index
    }

    /// Fill in a rectangle reserved with [`Self::reserve_rect`].
    pub fn set_reserved_rect(
        &mut self,
        index: usize,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) {
        // Same emission order as `push_quad`: v0 v1 v2 v0 v2 v3.
        let corners = [
            [x, y],
            [x, y + h],
            [x + w, y + h],
            [x, y],
            [x + w, y + h],
            [x + w, y],
        ];
        for (v, c) in self.vertices.iter_mut().skip(index).take(6).zip(corners) {
            v.position = c;
            v.color = color;
        }
    }

    /// Number of vertices queued so far this frame.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Add a circle to the draw list.
    pub fn draw_circle(&mut self, x: f32, y: f32, radius: f32, color: [f32; 4]) {
        self.push_quad(
            x,
            y,
            radius * 2.0,
            radius * 2.0,
            [0.0, 0.0],
            [1.0, 1.0],
            color,
            1,
        );
    }

    /// Add a straight line segment of the given thickness to the draw list.
    pub fn draw_line(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let len = dx.hypot(dy);
        if len <= f32::EPSILON {
            return;
        }
        // Unit normal scaled to half thickness, used to offset the segment into a quad.
        let half = thickness * 0.5;
        let (nx, ny) = (-dy / len * half, dx / len * half);
        self.push_quad_raw(
            [x0 + nx, y0 + ny],
            [x1 + nx, y1 + ny],
            [x1 - nx, y1 - ny],
            [x0 - nx, y0 - ny],
            color,
        );
    }

    /// Add a connected polyline (no miter joins — corners may show a small gap).
    pub fn draw_polyline(&mut self, points: &[[f32; 2]], thickness: f32, color: [f32; 4]) {
        for pair in points.windows(2) {
            self.draw_line(
                pair[0][0], pair[0][1], pair[1][0], pair[1][1], thickness, color,
            );
        }
    }

    /// Push an arbitrary (possibly rotated) quad from four corners, mode 0 (solid).
    fn push_quad_raw(
        &mut self,
        p0: [f32; 2],
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        color: [f32; 4],
    ) {
        self.begin_geometry();
        let v = |position: [f32; 2]| Vertex {
            position,
            uv: [0.0, 0.0],
            color,
            mode: 0,
        };
        self.vertices
            .extend_from_slice(&[v(p0), v(p1), v(p2), v(p0), v(p2), v(p3)]);
    }

    fn push_quad(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv_min: [f32; 2],
        uv_max: [f32; 2],
        color: [f32; 4],
        mode: u32,
    ) {
        self.begin_geometry();
        let v0 = Vertex {
            position: [x, y],
            uv: [uv_min[0], uv_min[1]],
            color,
            mode,
        };
        let v1 = Vertex {
            position: [x, y + h],
            uv: [uv_min[0], uv_max[1]],
            color,
            mode,
        };
        let v2 = Vertex {
            position: [x + w, y + h],
            uv: [uv_max[0], uv_max[1]],
            color,
            mode,
        };
        let v3 = Vertex {
            position: [x + w, y],
            uv: [uv_max[0], uv_min[1]],
            color,
            mode,
        };

        self.vertices.extend_from_slice(&[v0, v1, v2, v0, v2, v3]);
    }
}
