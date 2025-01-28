use std::ops::Index;
use egui::Vec2;
use glam::Vec3;
use petgraph::Direction;
use petgraph::graph::{DiGraph, UnGraph};
use petgraph::prelude::EdgeRef;
use rand::random;

#[derive(Default)]
#[derive(Copy)]
#[derive(Clone)]
pub struct Node {
    pub pos: Vec3
}

impl Node {
    pub fn new() -> Node {
        Node {
            pos: Vec3::new(random::<f32>() - 0.5, random::<f32>() - 0.5, random::<f32>() - 0.5)
        }
    }
}

pub(crate) struct World {
    repulsion: f32,
    center_attraction: f32,
    edge_strength: f32,
    graph: DiGraph<Node, ()>
}

impl World {
    pub fn new() -> Self {

        let mut g = DiGraph::<Node, ()>::new();
        for i in 0..100 {
            g.add_node(Node::new());
        }

        for i in 0..90 {
            let id_a = g.node_indices().nth(random::<usize>() % g.node_indices().len()).unwrap();
            let id_b = g.node_indices().nth(random::<usize>() % g.node_indices().len()).unwrap();
            g.add_edge(id_a, id_b, ());
        }

        Self {
            repulsion: 0.2,
            edge_strength: 20.0,
            center_attraction: 90.0,
            graph: g
        }
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

    pub fn update(&mut self) {
        let mut forces = Vec::new();

        for i in self.graph.node_indices() {
            let n = &self.graph[i];

            // Add node repulsion
            let mut force = Vec3::new(0.0, 0.0, 0.0);
            self.graph.raw_nodes().iter().for_each(|n2| {
                let diff = &n2.weight.pos - &n.pos;
                if diff.length() <= 0.0001 {
                    return;
                }
                force -= diff.normalize() * ( self.repulsion / diff.length() );
            });

            // Add edge attraction
            for e in self.graph.edges_directed(i, Direction::Outgoing) {
                let diff = &self.graph.node_weight(e.target()).unwrap().pos - &n.pos;

                if diff.length() <= 0.0001 {
                    continue;
                }

                force += diff.normalize() * diff.length() * self.edge_strength;
            }
            for e in self.graph.edges_directed(i, Direction::Incoming) {
                let diff = &self.graph.node_weight(e.source()).unwrap().pos - &n.pos;

                if diff.length() <= 0.0001 {
                    continue;
                }

                force += diff.normalize() * diff.length() * self.edge_strength;
            }

            force -= n.pos.normalize() * n.pos.length() * self.center_attraction;

            let delta = 1.0 / 420.;
            force *= delta;

            forces.push(force);
        }

        for (i, n) in self.graph.node_weights_mut().enumerate() {
            n.pos = n.pos + forces[i];
        }
    }

    pub fn get_mesh(&mut self) -> (Vec<Vec3>, Vec<(usize, usize)>) {
        let positions = self.graph.raw_nodes().iter().map(|n| n.weight.pos).collect();
        let edges = self.graph.raw_edges().iter().map(|e| {
            (e.source().index(), e.target().index())
        }).collect::<Vec<(usize, usize)>>();
        (positions, edges)
    }
}