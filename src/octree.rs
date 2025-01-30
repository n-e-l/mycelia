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
pub struct OctreeNode {
    bounds: Bounds,
    level: usize,
    children: Option<Box<[OctreeNode; 8]>>,
    center_of_mass: Vec3,
    total_mass: f32,
}

impl OctreeNode {
    pub fn new(center: Vec3, half_size: f32) -> OctreeNode {
        OctreeNode {
            bounds: Bounds { center, half_size},
            level: 0,
            children: None,
            center_of_mass: center,
            total_mass: 0.0,
        }
    }

    pub fn clear(&mut self) {
        self.children = None;
        self.center_of_mass = self.bounds.center;
        self.total_mass = 0.0;
    }

    fn repulsion(p1: &Vec3, m1: f32, p2: &Vec3, m2: f32, repulsion: f32) -> Vec3 {
        let mut force = Vec3::ZERO;
        let diff = (p2 - p1);
        if diff.length() >= 0.01 {
            force = -diff.normalize() * m1 * m2 * repulsion / ( diff.length() * diff.length());
        }
        force
    }

    pub fn get_force(&self, point: &Vec3, repulsion: &f32, theta: &f32, mut force: &mut Vec3) {
        if let Some(children) = &self.children {
            for i in 0..children.len() {
                let node_theta = ( children[i].bounds.half_size * 2.) / children[i].bounds.center.distance(*point);
                if node_theta < *theta {
                    if children[ i ].total_mass == 0.0 { continue; }
                    *force += Self::repulsion(point, 1., &children[i].center_of_mass, children[i].total_mass, *repulsion);
                } else {
                    children[i].get_force(point, repulsion, theta, force);
                }
            }
        } else {
            *force += Self::repulsion(point, 1., &self.center_of_mass, self.total_mass, *repulsion);
        }
    }

    pub fn insert(&mut self, position: Vec3) -> bool {

        if !self.bounds.contains(position) {
            return false;
        }

        if self.level > 8 {
            let new_total_mass = self.total_mass + 1.;
            self.center_of_mass = (self.center_of_mass * self.total_mass + position * 1.) / new_total_mass;
            self.total_mass = new_total_mass;
            return true;
        }

        if self.total_mass == 0. && self.children.is_none() {
            // Just store the data
            self.center_of_mass = position;
            self.total_mass = 1.;
            return true;
        }

        if self.total_mass > 0. && self.children.is_none() {
            // Move the existing data into a child
            self.subdivide();
            self.insert_into_children(self.center_of_mass);
        }

        let new_total_mass = self.total_mass + 1.;
        self.center_of_mass = (self.center_of_mass * self.total_mass + position * 1.) / new_total_mass;
        self.total_mass = new_total_mass;

        self.insert_into_children(position)
    }

    fn subdivide(&mut self) {
        let half = self.bounds.half_size / 2.0;
        let center = self.bounds.center;

        let mut children: Vec<OctreeNode> = vec![];
        for i in 0..8 {
            let x = if i & 1 == 0 { center.x - half } else { center.x + half };
            let y = if i & 2 == 0 { center.y - half } else { center.y + half };
            let z = if i & 4 == 0 { center.z - half } else { center.z + half };
            let mut child = OctreeNode::new(Vec3::new(x, y, z), half);
            child.level = self.level + 1;
            children.push(child);
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

    fn insert_into_children(&mut self, position: Vec3) -> bool {
        if let Some(ref mut children) = self.children.as_mut() {
            let index = self.bounds.get_octant(position);
            return children[index].insert(position);
        }
        false
    }
}