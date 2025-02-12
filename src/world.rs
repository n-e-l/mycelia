use std::ops::Index;
use std::time::Instant;
use glam::Vec3;
use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex, NodeWeightsMut, UnGraph};
use petgraph::prelude::EdgeRef;
use rand::random;

#[derive(Default)]
#[derive(Copy)]
#[derive(Clone)]
pub struct Node {
    pub pos: Vec3,
    pub selected: bool
}

impl Node {
    pub fn new() -> Node {
        Node {
            pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, random::<f32>() - 0.5) * 0.3,
            selected: false
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
        for i in 0..1000 {
            g.add_node(Node::new());
        }

        for i in 0..100 {
            let id_a = g.node_indices().nth(random::<usize>() % g.node_indices().len()).unwrap();
            let id_b = g.node_indices().nth(random::<usize>() % g.node_indices().len()).unwrap();
            g.update_edge(id_a, id_b, ());
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

    pub fn nodes(&mut self) -> NodeWeightsMut<Node> {
        self.graph.node_weights_mut()
    }
}