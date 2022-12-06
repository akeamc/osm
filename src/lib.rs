use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use geo::Point;
use osmpbfreader::{Node, NodeId, OsmObj, OsmPbfReader, Relation, RelationId, Way, WayId};

#[derive(Debug, Default)]
pub struct Planet {
    pub nodes: HashMap<NodeId, Node>,
    pub ways: HashMap<WayId, Way>,
    pub relations: HashMap<RelationId, Relation>,
}

impl Planet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, obj: OsmObj) {
        match obj {
            OsmObj::Node(n) => {
                self.nodes.insert(n.id, n);
            }
            OsmObj::Way(w) => {
                self.ways.insert(w.id, w);
            }
            OsmObj::Relation(r) => {
                self.relations.insert(r.id, r);
            }
        }
    }

    pub fn node_coords(&self, node: &NodeId) -> Option<Point> {
        self.nodes
            .get(node)
            .map(|node| Point::new(node.lon(), node.lat()))
    }

    pub fn relation_polygon(&self, rel: &RelationId) -> Option<Vec<Vec<NodeId>>> {
        let rel = self.relations.get(rel)?;

        let mut ways: Vec<&[NodeId]> = rel
            .refs
            .iter()
            .map(|r| {
                let w = r.member.way()?;
                let w = self.ways.get(&w)?;

                if w.nodes.len() < 2 {
                    None
                } else {
                    Some(&w.nodes[..])
                }
            })
            .collect::<Option<Vec<_>>>()?;

        if ways.is_empty() {
            return None;
        }

        let mut closed_rings: Vec<Vec<NodeId>> = Vec::new();

        while !ways.is_empty() {
            let mut ring = ways.pop().unwrap().to_vec();

            while ring.first() != ring.last() {
                let (i, way) = ways
                    .iter()
                    .enumerate()
                    .find(|(_i, w)| ring.first() == w.last() || ring.last() == w.first())
                    // .expect("cannot close ring!");
                    ?;

                if ring.first() == way.last() {
                    ring = [way, &ring[1..]].concat();
                } else {
                    ring.extend_from_slice(&way[1..]);
                }

                ways.swap_remove(i);
            }

            closed_rings.push(ring);
        }

        Some(closed_rings)
    }
}

pub fn read(path: impl AsRef<Path>) -> anyhow::Result<Planet> {
    let mut pbf = OsmPbfReader::new(BufReader::with_capacity(1024 * 1024, File::open(path)?));

    let mut planet = Planet::new();

    for res in pbf.par_iter() {
        planet.insert(res?);
    }

    println!("built planet");

    Ok(planet)
}
