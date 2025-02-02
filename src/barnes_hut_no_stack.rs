use glam::{Vec3, Vec4};
use crate::barnes_hut::OctreeNode;

#[derive(Debug)]
#[derive(Copy)]
#[derive(Clone)]
struct Bounds {
    center: Vec3,
    size: f32,
}

impl Bounds {
    fn contains(&self, point: Vec3) -> bool {
        point.x >= self.center.x - self.size && point.x <= self.center.x + self.size &&
        point.y >= self.center.y - self.size && point.y <= self.center.y + self.size &&
        point.z >= self.center.z - self.size && point.z <= self.center.z + self.size
    }
    
    fn into_octant(mut self, i: usize) -> Self {
        self.size *= 0.5;
        self.center.x += (-0.5 + (i & 1) as f32) * self.size;
        self.center.y += (-0.5 + (i >> 1 & 1) as f32) * self.size;
        self.center.z += (-0.5 + (i >> 2 & 1) as f32) * self.size;
        self
    }

    fn into_octants(&self) -> [Bounds; 8] {
        [0, 1, 2, 3, 4, 5, 6, 7].map(|i| self.into_octant(i))
    }

    fn get_octant(&self, point: &Vec3) -> usize {
        let mut index = 0;
        if point.x > self.center.x { index |= 1 }
        if point.y > self.center.y { index |= 2 }
        if point.z > self.center.z { index |= 4 }
        index
    }
}

#[derive(Debug)]
#[derive(Copy)]
#[derive(Clone)]
struct Node {
    bounds: Bounds,
    children: usize,
    center_of_mass: Vec3,
    mass: f32,
    next: usize,
}

impl Node {
    fn is_leaf(&self) -> bool { self.children == 0 }
    fn is_empty(&self) -> bool { self.mass == 0. }
}

#[derive(Debug)]
pub struct Octree {
    nodes: Vec<Node>,
    center: Vec3,
    size: f32,
}

impl Octree {
    pub fn new(center: Vec3, size: f32) -> Octree {
        Octree {
            center,
            size,
            nodes: vec![ Node {
                bounds: Bounds { center, size},
                children: 0,
                center_of_mass: Vec3::ZERO,
                mass: 0.,
                next: 0,
            }]
        }
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.nodes.push( Node {
            bounds: Bounds { center: self.center, size: self.size},
            children: 0,
            center_of_mass: Vec3::ZERO,
            mass: 0.,
            next: 0,
        });
    }

    fn repulsion(p1: &Vec3, m1: f32, p2: &Vec3, m2: f32, repulsion: f32, out: &mut Vec3) {
        if m1 == 0. || m2 == 0. { return; }

        let diff = p2 - p1;
        let l = diff.length();
        if l >= 0.01 {
            *out -= diff.normalize() * m1 * m2 * repulsion / ( l * l );
        }
    }

    pub fn get_force(&self, point: &Vec3, mass: f32, repulsion: f32, max_theta: f32) -> Vec3 {
        let mut force = Vec3::ZERO;

        // Go through the nodes
        let mut node = 1;
        loop {
            let n = self.nodes[node];

            if n.is_leaf() || n.bounds.size / (n.bounds.center - point).length() < max_theta {
                // Add the mass
                Self::repulsion(point, mass, &n.center_of_mass, n.mass, repulsion, &mut force);

                // Skip children
                node = n.next;

                if node == 0 {
                    break;
                }

                continue;
            } else {
                // Go deeper
                node = n.children;
            }
        }

        force
    }

    pub fn insert(&mut self, position: Vec3, mass: f32) {
        let mut node = 0;

        // Get corresponding leaf
        while !self.nodes[node].is_leaf() {
            let octant = self.nodes[node].bounds.get_octant(&position);
            node = self.nodes[node].children + octant;
        }

        // Insert the data
        if self.nodes[node].is_empty() {
            self.nodes[node].mass = mass;
            self.nodes[node].center_of_mass = position;
            return;
        }

        let p = self.nodes[node].center_of_mass;
        let m = self.nodes[node].mass;

        // Same positions
        if p == position {
            self.nodes[node].mass += mass;
            return;
        }

        // Split up leaf node and move data
        self.nodes[node].children = self.subdivide(node);
        self.nodes[node].center_of_mass = Vec3::ZERO;
        self.nodes[node].mass = 0.;

        loop {
            let o1 = self.nodes[node].bounds.get_octant(&position);
            let o2 = self.nodes[node].bounds.get_octant(&p);

            if o1 == o2 {
                node = self.nodes[node].children + o1;
                self.nodes[node].children = self.subdivide(node);
                continue;
            }

            let c = self.nodes[node].children;
            self.nodes[c + o1].mass = mass;
            self.nodes[c + o2].mass = m;
            self.nodes[c + o1].center_of_mass = position;
            self.nodes[c + o2].center_of_mass = p;
            break;
        }
    }

    /**
     * Subdivide a node
     */
    fn subdivide(&mut self, node: usize) -> usize {
        let children = self.nodes.len();

        let parent_next = self.nodes[node].next;
        self.nodes[node].bounds.into_octants().iter().enumerate().for_each(|(index, bounds)| {
            self.nodes.push(
            Node {
                bounds: *bounds,
                children: 0,
                center_of_mass: Vec3::ZERO,
                mass: 0.,
                next: if index == 7 { parent_next } else { children + index + 1 },
            });
        });

        children
    }

    pub fn backpropagate(&mut self) {
        for node in (0..self.nodes.len()).rev() {
            if self.nodes[node].is_leaf() {
                continue;
            }

            let c = self.nodes[node].children;
            self.nodes[node].center_of_mass = (0..8).into_iter().map(|i|
                self.nodes[c + i].center_of_mass * self.nodes[c + i].mass
            ).sum();
            self.nodes[node].mass = (0..8).into_iter().map(|i|
                self.nodes[c + i].mass
            ).sum();

            let mass = self.nodes[node].mass;
            self.nodes[node].center_of_mass /= mass;

        }
    }

}