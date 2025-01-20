use glam::Vec3;
use rand::random;

struct Node {
    pos: Vec3
}

pub(crate) struct Graph {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
}

impl Graph {
    pub fn new() -> Self {
        let mut nodes = vec![];
        for _ in 0..60 {
            nodes.push(Node {pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, 0.0)});
        }
        Self {
            nodes,
            edges: Vec::new(),
        }
    }

    pub fn set_count(&mut self, count: usize) {

        if count < self.nodes.len() {
            for _ in count..self.nodes.len() {
                self.nodes.pop();
            }
        } else {
            for _ in self.nodes.len()..count {
                self.nodes.push(Node {pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, 0.0) * 0.1});
            }
        }
    }

    pub fn reset(&mut self) {
        for node in self.nodes.iter_mut() {
            node.pos = Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, 0.0);
        }
    }

    pub fn update(&mut self) {
        let delta = 0.01 / 120.0;

        let mut new_nodes = vec![];
        for i in 0..self.nodes.len() {
            let node = &self.nodes[i];

            let mut force: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
            for j in 0..self.nodes.len() {
                if i == j { continue }

                let diff = &self.nodes[j].pos - &node.pos;
                force -= diff.normalize() * ( 0.1 / diff.length() );
            }

            force -= node.pos.normalize() * node.pos.length() * 90.;
            force *= delta;

            let new_node = Node { pos: node.pos + force };
            new_nodes.push(new_node);
        }

        self.nodes = new_nodes;
    }

    pub fn get_positions(&self) -> Vec<Vec3> {
        self.nodes.iter()
            .map(|n| n.pos)
            .collect::<Vec<Vec3>>()
    }
}