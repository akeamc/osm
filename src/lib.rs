use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use geo::Point;
use osmpbfreader::{Node, NodeId, OsmId, OsmObj, OsmPbfReader, Relation, RelationId, Way, WayId};

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

    pub fn obj_coords(&self, obj: &OsmId) -> Option<Point> {
        match obj {
            OsmId::Node(n) => self.node_coords(n),
            OsmId::Way(w) => self.node_coords(self.ways.get(w)?.nodes.first()?),
            OsmId::Relation(_) => todo!(),
        }
    }

    pub fn node_coords(&self, node: &NodeId) -> Option<Point> {
        self.nodes
            .get(node)
            .map(|node| Point::new(node.lon(), node.lat()))
    }

    pub fn relation_rings(&self, rel: &RelationId) -> Option<Vec<Vec<NodeId>>> {
        let rel = self.relations.get(rel)?;

        let mut ways: Vec<&[NodeId]> = rel
            .refs
            .iter()
            .filter_map(|r| match r.member {
                osmpbfreader::OsmId::Node(_) => None,
                osmpbfreader::OsmId::Way(w) => match r.role.as_str() {
                    "outer" => {
                        let w = self.ways.get(&w)?;
                        assert!(w.nodes.len() >= 2);
                        Some(&w.nodes[..])
                    }
                    "inner" => {
                        // panic!()
                        eprintln!("inner!!!! ({:?})", rel.id.0);
                        None
                    }
                    _ => panic!(),
                },
                osmpbfreader::OsmId::Relation(_) => {
                    assert_eq!(r.role, "subarea");
                    None
                }
            })
            .collect::<Vec<_>>();

        if ways.is_empty() {
            eprintln!("EMPTY! (relation {})", rel.id.0);
            return None;
        }

        let mut closed_rings: Vec<Vec<NodeId>> = Vec::new();

        while !ways.is_empty() {
            let mut ring = ways.pop().unwrap().to_vec();

            while ring.first() != ring.last() {
                let (i, w) = ways
                    .iter()
                    .enumerate()
                    .find(|(_i, w)| ring.last() == w.first() || ring.last() == w.last())
                    // .unwrap();
                    ?;

                if ring.last() == w.first() {
                    ring.extend_from_slice(&w[1..]);
                } else {
                    ring.extend(w.iter().rev().skip(1));
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

    eprintln!("built planet");

    Ok(planet)
}
