use std::ops::Index;
use std::time::Instant;
use egui::Vec2;
use glam::Vec3;
use petgraph::Direction;
use petgraph::graph::{DiGraph, NodeIndex, NodeWeightsMut, UnGraph};
use petgraph::prelude::EdgeRef;
use rand::random;
use crate::barnes_hut::OctreeNode;
use crate::barnes_hut_no_stack;

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
    repulsion: f32,
    center_attraction: f32,
    edge_strength: f32,
    graph: DiGraph<Node, ()>,
    octree: OctreeNode,
    octree_l: barnes_hut_no_stack::Octree,
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

        let mut octree = OctreeNode::new(Vec3::new(0., 0., 0.), 2.6);
        for i in g.node_indices() {
            let n = g.node_weight(i).unwrap();
            octree.insert(n.pos);
        }

        let mut octree_l = barnes_hut_no_stack::Octree::new(Vec3::new(0., 0., 0.), 2.6);
        for i in g.node_indices() {
            let n = g.node_weight(i).unwrap();
            octree_l.insert(n.pos, 1.);
        }

        Self {
            repulsion: 0.2,
            edge_strength: 20.0,
            center_attraction: 20000.0,
            graph: g,
            octree,
            octree_l,
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

    pub fn get_octree(&self) -> &OctreeNode {
        &self.octree
    }

    pub fn get_bh_theta(&mut self) -> &mut f32 {
        &mut self.bh_theta
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

        if !self.run_physics {
            return;
        }

        if self.bh_physics {
            self.octree_l.clear();
            for i in self.graph.node_indices() {
                let n = self.graph.node_weight(i).unwrap();
                self.octree_l.insert(n.pos, 1.);
            }
            self.octree_l.backpropagate();
        } else {
            // self.octree.clear();
            // for i in self.graph.node_indices() {
            //     let n = self.graph.node_weight(i).unwrap();
            //     self.octree.insert(n.pos);
            // }
        }

        use rayon::prelude::*;
        let mut forces: Vec<(NodeIndex, Vec3)> = self.graph.node_indices().enumerate().par_bridge().map(|(index, i)| {

            let n = &self.graph[i];

            let mut force = Vec3::new(0.0, 0.0, 0.0);

            // Add node repulsion
            let start = Instant::now();
            if self.bh_physics {
                force += self.octree_l.get_force(&n.pos, 1., self.repulsion, self.bh_theta);
            } else {
                // self.octree.get_force(&n.pos, &self.repulsion, &self.bh_theta, &mut force);
                self.graph.raw_nodes().iter().for_each(|n2| {
                    let diff = &n2.weight.pos - &n.pos;
                    if diff.length() <= 0.01 {
                        return;
                    }
                    force -= diff.normalize() * (self.repulsion / ( diff.length() * diff.length() ));
                });
            }
            let duration = start.elapsed();

            // Add edge attraction
            for e in self.graph.edges_directed(i, Direction::Outgoing) {
                let diff = &self.graph.node_weight(e.target()).unwrap().pos - &n.pos;

                if diff.length() <= 0.01 {
                    continue;
                }

                force += diff.normalize() * diff.length() * self.edge_strength;
            }
            for e in self.graph.edges_directed(i, Direction::Incoming) {
                let diff = &self.graph.node_weight(e.source()).unwrap().pos - &n.pos;

                if diff.length() <= 0.01 {
                    continue;
                }

                force += diff.normalize() * diff.length() * self.edge_strength;
            }

            force -= n.pos.normalize() * n.pos.length() * self.center_attraction;

            let delta = 1.0 / 402000.;
            force *= delta;

            (i, force)
        }).collect();

        for (i, n) in forces {
            self.graph.node_weight_mut(i).unwrap().pos += n;
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