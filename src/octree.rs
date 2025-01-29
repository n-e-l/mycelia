use glam::{Vec3, Vec4};

#[derive(Debug)]
struct Bounds {
    center: Vec3,
    half_size: f32,
}

impl Bounds {
    fn contains(&self, point: Vec3) -> bool {
        point.x >= self.center.x - self.half_size && point.x <= self.center.x + self.half_size &&
        point.y >= self.center.y - self.half_size && point.y <= self.center.y + self.half_size &&
        point.z >= self.center.z - self.half_size && point.z <= self.center.z + self.half_size
    }

    fn get_octant(&self, point: Vec3) -> usize {
        let mut index = 0;
        if point.x > self.center.x { index |= 1 }
        if point.y > self.center.y { index |= 2 }
        if point.z > self.center.z { index |= 4 }
        index
    }
}

#[derive(Debug)]
pub struct OctreeNode<T> {
    bounds: Bounds,
    data: Option<(T, Vec3)>,
    children: Option<Box<[OctreeNode<T>; 8]>>,
}

impl<T: std::fmt::Debug> OctreeNode<T> {
    pub fn new(center: Vec3, half_size: f32) -> OctreeNode<T> {
        OctreeNode {
            bounds: Bounds { center, half_size},
            data: None,
            children: None,
        }
    }

    pub fn insert(&mut self, node: T, position: Vec3) -> bool {
        if !self.bounds.contains(position) {
            return false;
        }

        if self.data.is_none() && self.children.is_none() {
            // Just store the data
            self.data = Some((node, position));
            return true;
        }

        if self.data.is_some() && self.children.is_none() {
            // Move the existing data into a child
            let existing_data = self.data.take().unwrap();
            self.subdivide();
            return self.insert_into_children(existing_data.0, existing_data.1);
        }

        self.insert_into_children(node, position);
        true
    }

    fn subdivide(&mut self) {
        let half = self.bounds.half_size / 2.0;
        let center = self.bounds.center;

        let mut children: Vec<OctreeNode<T>> = vec![];
        for i in 0..8 {
            let x = if i & 1 == 0 { center.x - half } else { center.x + half };
            let y = if i & 2 == 0 { center.y - half } else { center.y + half };
            let z = if i & 4 == 0 { center.z - half } else { center.z + half };
            children.push(OctreeNode::new(Vec3::new(x, y, z), half));
        }

        self.children = Some(Box::new(children.try_into().unwrap()));
    }

    pub fn mesh_lines(&self) -> Vec<(Vec4, Vec4)> {
        let mut lines: Vec<(Vec4, Vec4)> = vec![];
        let half_size = self.bounds.half_size;
        let center = Vec4::new(self.bounds.center.x, self.bounds.center.y, self.bounds.center.z, 0.);

        // X
        lines.push((center + Vec4::new(-half_size, -half_size, -half_size, half_size), center + Vec4::new(half_size, -half_size, -half_size, half_size)));
        lines.push((center + Vec4::new(-half_size, half_size, -half_size, half_size), center + Vec4::new(half_size, half_size, -half_size, half_size)));
        lines.push((center + Vec4::new(-half_size, -half_size, half_size, half_size), center + Vec4::new(half_size, -half_size, half_size, half_size)));
        lines.push((center + Vec4::new(-half_size, half_size, half_size, half_size), center + Vec4::new(half_size, half_size, half_size, half_size)));

        // Y
        lines.push((center + Vec4::new(-half_size, -half_size, -half_size, half_size), center + Vec4::new(-half_size, half_size, -half_size, half_size)));
        lines.push((center + Vec4::new(half_size, -half_size, -half_size, half_size), center + Vec4::new(half_size, half_size, -half_size, half_size)));
        lines.push((center + Vec4::new(-half_size, -half_size, half_size, half_size), center + Vec4::new(-half_size, half_size, half_size, half_size)));
        lines.push((center + Vec4::new(half_size, -half_size, half_size, half_size), center + Vec4::new(half_size, half_size, half_size, half_size)));

        // Z
        lines.push((center + Vec4::new(-half_size, -half_size, -half_size, half_size), center + Vec4::new(-half_size, -half_size, half_size, half_size)));
        lines.push((center + Vec4::new(half_size, -half_size, -half_size, half_size), center + Vec4::new(half_size, -half_size, half_size, half_size)));
        lines.push((center + Vec4::new(-half_size, half_size, -half_size, half_size), center + Vec4::new(-half_size, half_size, half_size, half_size)));
        lines.push((center + Vec4::new(half_size, half_size, -half_size, half_size), center + Vec4::new(half_size, half_size, half_size, half_size)));

        if let Some(children) = &self.children {

            // lines.push((center + Vec4::new(-half_size, 0., 0., half_size), center + Vec4::new(half_size, 0., 0., half_size)));
            // lines.push((center + Vec4::new(0., -half_size, 0., half_size), center + Vec4::new(0., half_size, 0., half_size)));
            // lines.push((center + Vec4::new(0., 0., -half_size, half_size), center + Vec4::new(0., 0., half_size, half_size)));

            for i in 0..8 {
                lines.append(&mut children[i].mesh_lines());
            }
        }

        lines
    }

    fn insert_into_children(&mut self, node: T, position: Vec3) -> bool {
        if let Some(ref mut children) = self.children.as_mut() {
            let index = self.bounds.get_octant(position);
            return children[index].insert(node, position);
        }
        false
    }
}