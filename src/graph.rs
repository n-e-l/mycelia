use glam::Vec3;
use rand::random;

pub struct Node {
    pub pos: Vec3
}

pub(crate) struct Graph {
    nodes: Vec<Node>,
    edges: Vec<(usize, usize)>,
    repulsion: f32,
    center_attraction: f32,
    edge_strength: f32
}

impl Graph {
    pub fn new() -> Self {
        let mut nodes = vec![];
        let mut edges = vec![];

        Self {
            nodes,
            edges,
            repulsion: 0.2,
            edge_strength: 20.0,
            center_attraction: 90.0
        }
    }

    pub fn randomize(&mut self) {
        self.nodes.clear();
        self.edges.clear();

        for _ in 0..200 {
            self.nodes.push(Node {pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, random::<f32>() - 0.5)});
        }

        for _ in 0..150 {
            self.edges.push((random::<usize>() % self.nodes.len(), random::<usize>() % self.nodes.len()));
        }

        for i in (0..self.nodes.len()).rev() {
            let mut has_edge = false;
            for e in self.edges.iter() {
                if e.0 == i || e.1 == i {
                    println!("Node {} has edge", i);
                    has_edge = true;
                    break;
                }
            }
            if !has_edge {
                println!("Delete node {}", i);
                self.delete_node(i);
            }
        }
    }

    pub fn delete_node(&mut self, id: usize) {
        self.nodes.remove(id);

        let mut rem_edges: Vec<usize> = vec![];
        for (edge_id, edge) in self.edges.iter_mut().enumerate() {
            if edge.0 == id || edge.1 == id {
                rem_edges.push(edge_id);
            }
            if edge.0 > id {
                edge.0 = edge.0 - 1;
            }
            if edge.1 > id {
                edge.1 = edge.1 - 1;
            }
        }

        for e in rem_edges.iter().rev() {
            println!("Delete edge {}", e);
            self.edges.remove(*e);
        }
    }

    pub fn set_repulsion(&mut self, repulsion: f32) {
        self.repulsion = repulsion;
    }

    pub fn set_edge_strength(&mut self, edge_strength: f32) {
        self.edge_strength = edge_strength;
    }

    pub fn get_repulsion(&mut self) -> &mut f32 {
        &mut self.repulsion
    }

    pub fn get_center_attraction_mut(&mut self) -> &mut f32 {
        &mut self.center_attraction
    }

    pub fn get_edge_strength(&mut self) -> &mut f32 {
        &mut self.edge_strength
    }

    pub fn add_node(&mut self) {
        self.nodes.push(Node {pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, 0.0)});
    }

    pub fn add_edge(&mut self, a: usize, b: usize) {
        self.edges.push((a, b));
    }

    pub fn set_count(&mut self, count: usize) {

        if count < self.nodes.len() {
            for _ in count..self.nodes.len() {
                self.nodes.pop();
            }
        } else {
            for _ in self.nodes.len()..count {
                self.nodes.push(Node {pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, random::<f32>() - 0.5) * 0.1});
            }
        }
    }

    pub fn reset(&mut self) {
        self.nodes.clear();
        self.edges.clear();
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
                if diff.length() <= 0.01 {
                    continue;
                }
                force -= diff.normalize() * ( self.repulsion / diff.length() );
            }

            force -= node.pos.normalize() * node.pos.length() * self.center_attraction;

            // Add edge forces
            for e in &self.edges {
                if e.0 == i || e.1 == i {
                    let diff = if e.0 == i {
                        &self.nodes[e.1].pos - &node.pos
                    } else {
                        &self.nodes[e.0].pos - &node.pos
                    };
                    if diff.length() <= 0.01 {
                        continue;
                    }
                    force += diff.normalize() * diff.length() * self.edge_strength;
                }
            }

            force *= delta;

            let new_node = Node { pos: node.pos + force };
            new_nodes.push(new_node);
        }

        self.nodes = new_nodes;
    }

    pub fn get_nodes_mut(&mut self) -> &mut Vec<Node> {
        &mut self.nodes
    }

    pub fn get_positions(&self) -> Vec<Vec3> {
        self.nodes.iter()
            .map(|n| n.pos)
            .collect::<Vec<Vec3>>()
    }

    pub fn get_edges(&self) -> &Vec<(usize, usize)> {
        &self.edges
    }
}