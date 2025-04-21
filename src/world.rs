use std::ops::Index;
use std::time::Instant;
use glam::Vec3;
use petgraph::{Directed, Direction};
use petgraph::data::Build;
use petgraph::graph::{DiGraph, Edge, Edges, NodeIndex, NodeWeightsMut, UnGraph};
use petgraph::prelude::EdgeRef;
use petgraph::visit::{IntoEdges, IntoEdgesDirected, NodeCount};
use rand::random;

#[derive(Default)]
#[derive(Copy)]
#[derive(Clone)]
pub struct Node {
    pub pos: Vec3,
    pub level: f32
}

impl Node {
    pub fn new_random(level: f32) -> Node {
        Node {
            pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, random::<f32>() - 0.5) * 0.3,
            level
        }
    }

    pub fn new(pos: Vec3, level: f32) -> Node {
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

        let nodes: usize = 400;
        for i in 0..nodes {
            g.add_node(Node::new_random(0.));
        }

        for i in 0..( nodes as f32 * 1.04 ) as usize {
            let i_a = i % nodes;
            // let i_a = random::<usize>() % nodes;
            let i_b = random::<usize>() % nodes;
            if i_a == i_b { continue; }
            g.add_edge(NodeIndex::new(i_a), NodeIndex::new(i_b), ());
        }

        // for x in 0..100 {
        //     for y in 0..10 {
        //         let mut index = g.node_count();
        //         g.add_node(Node::new(Vec3::new(x as f32, y as f32, -0.5) / 10. + Vec3::new(-0.5, -0.5, 0.), 0));
        //
        //         for z in 0..9 {
        //
        //             g.add_node(Node::new(Vec3::new(x as f32, y as f32, z as f32 - 5.) / 10. + Vec3::new(-0.5, -0.5, 0.1), z + 1));
        //
        //             // Add an edge to the child
        //             let id_a = g.node_indices().nth(index).unwrap();
        //             let id_b = g.node_indices().nth(index + 1).unwrap();
        //             g.update_edge(id_a, id_b, ());
        //             index += 1;
        //         }
        //     }
        // }

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

        let g2 = self.graph.clone();
        g2.node_indices().for_each(|i| {
            let w = g2.node_weight(i).unwrap().level;
            if w == 0.0 {
                return;
            }

            let count = g2.edges_directed(i, Direction::Outgoing).count();
            if count == 0 { return; }

            g2.edges_directed(i, Direction::Outgoing).for_each(|edge| {
                self.graph.node_weight_mut(edge.target()).unwrap().level = 1.;
            });
            self.graph.node_weight_mut(i).unwrap().level = 0.;
        });

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