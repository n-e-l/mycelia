use std::ops::Index;
use std::time::Instant;
use glam::Vec3;
use petgraph::{Directed, Direction};
use petgraph::graph::{DiGraph, Edge, Edges, NodeIndex, NodeWeightsMut, UnGraph};
use petgraph::prelude::EdgeRef;
use petgraph::visit::NodeCount;
use rand::random;

#[derive(Default)]
#[derive(Copy)]
#[derive(Clone)]
pub struct Node {
    pub pos: Vec3,
    pub level: u32
}

impl Node {
    pub fn new_random(level: u32) -> Node {
        Node {
            pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, random::<f32>() - 0.5) * 0.3,
            level
        }
    }

    pub fn new(pos: Vec3, level: u32) -> Node {
        Node {
            pos,
            level
        }
    }
}

pub(crate) struct World {
    center_attraction: f32,
    edge_strength: f32,
    graph: DiGraph<Node, ()>,
    bh_physics: bool,
    bh_theta: f32,
    run_physics: bool,
}

impl World {
    pub fn new() -> Self {

        let mut g = DiGraph::<Node, ()>::new();
        g.add_node(Node::new_random(0));

        let layers = vec![3, 3, 3];
        let mut index = 0;
        let mut stack: Vec<(usize, usize)> = vec![];
        let mut child_index = 0;
        loop {

            // Step out
            let node_count = layers[stack.len()];
            if child_index > node_count {
                if let Some((parent, child)) = stack.pop() {
                    index = parent;
                    child_index = child;
                    continue;
                } else {
                    // Done
                    break;
                }
            }

            // Add child_node
            g.add_node(Node::new_random(stack.len() as u32 + 1));
            let child_array_index = g.node_count() - 1;

            // Add an edge to the child
            let id_a = g.node_indices().nth(index).unwrap();
            let id_b = g.node_indices().nth(child_array_index).unwrap();
            g.update_edge(id_a, id_b, ());

            // Add the child's childs if needed
            if stack.len() + 1 < layers.len() {
                stack.push((index, child_index + 1));
                index = child_array_index;
                child_index = 0; // Deeper we always set zero, because we only return when all the childs are added
                continue;
            }

            child_index = child_index + 1;
        }

        Self {
            edge_strength: 20.0,
            center_attraction: 20000.0,
            graph: g,
            bh_physics: false,
            bh_theta: 0.5,
            run_physics: true
        }
    }

    pub fn bh_physics(&mut self) -> &mut bool {
        &mut self.bh_physics
    }

    pub fn run_physics(&mut self) -> &mut bool {
        &mut self.run_physics
    }

    pub fn get_bh_theta(&mut self) -> &mut f32 {
        &mut self.bh_theta
    }

    pub fn get_center_attraction_mut(&mut self) -> &mut f32 {
        &mut self.center_attraction
    }

    pub fn get_edge_strength(&mut self) -> &mut f32 {
        &mut self.edge_strength
    }

    pub fn update(&mut self) {

        if !self.run_physics {
            return;
        }

    }

    pub fn get_mesh(&mut self) -> (Vec<Node>, Vec<(usize, usize)>) {
        let positions = self.graph.raw_nodes().iter().map(|n| n.weight).collect::<Vec<_>>();
        let edges = self.graph.raw_edges().iter().map(|e| {
            (e.source().index(), e.target().index())
        }).collect::<Vec<(usize, usize)>>();
        (positions, edges)
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn nodes(&self) -> Vec<&Node> {
        self.graph.node_weights().collect::<Vec<&Node>>()
    }

    pub fn edges(&self) -> &[Edge<()>] {
        self.graph.raw_edges()
    }

    pub fn nodes_mut(&mut self) -> NodeWeightsMut<Node> {
        self.graph.node_weights_mut()
    }
}