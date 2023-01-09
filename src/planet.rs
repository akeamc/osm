use std::{
    collections::HashMap,
    io::{BufReader, Read},
};

use geo::{Centroid, LineString, Point};
use osmpbfreader::{NodeId, OsmId, OsmObj, OsmPbfReader, Relation, RelationId, Tags};

use self::{nodes::Nodes, ways::Ways};

#[derive(Debug, Default)]
pub struct Meta {
    pub name: Option<String>,
}

impl From<Tags> for Meta {
    fn from(tags: Tags) -> Self {
        if !tags.contains_key("amenity") && !tags.contains_key("building") {
            return Default::default();
        }

        let name = tags.get("name").map(|s| s.to_string());

        Self { name }
    }
}

pub mod nodes {
    use std::collections::HashMap;

    use osmpbfreader::NodeId;

    use super::Meta;

    #[derive(Debug)]
    pub struct Node {
        // pub tags: Tags,
        pub meta: Meta,
        pub decimicro_lat: i32,
        pub decimicro_lon: i32,
    }

    impl From<osmpbfreader::Node> for Node {
        fn from(
            osmpbfreader::Node {
                id: _,
                tags,
                decimicro_lat,
                decimicro_lon,
            }: osmpbfreader::Node,
        ) -> Self {
            Self {
                // tags,
                meta: tags.into(),
                decimicro_lat,
                decimicro_lon,
            }
        }
    }

    impl Node {
        #[inline]
        pub fn lat(&self) -> f64 {
            self.decimicro_lat as f64 * 1e-7
        }

        #[inline]
        pub fn lon(&self) -> f64 {
            self.decimicro_lon as f64 * 1e-7
        }
    }

    pub type Nodes = HashMap<NodeId, Node>;
}

pub mod ways {
    use std::collections::HashMap;

    use osmpbfreader::{NodeId, WayId};

    use super::Meta;

    pub struct Way {
        pub meta: Meta,
        pub nodes: Vec<NodeId>,
    }

    impl From<osmpbfreader::Way> for Way {
        fn from(osmpbfreader::Way { id: _, tags, nodes }: osmpbfreader::Way) -> Self {
            Way {
                meta: tags.into(),
                nodes,
            }
        }
    }

    pub type Ways = HashMap<WayId, Way>;
}

#[derive(Default)]
pub struct Planet {
    pub nodes: Nodes,
    pub ways: Ways,
    pub relations: HashMap<RelationId, Relation>,
}

impl Planet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, obj: OsmObj) {
        match obj {
            OsmObj::Node(n) => {
                self.nodes.insert(n.id, n.into());
            }
            OsmObj::Way(w) => {
                self.ways.insert(w.id, w.into());
            }
            OsmObj::Relation(r) => {
                self.relations.insert(r.id, r);
            }
        }
    }

    pub fn obj_coords(&self, obj: &OsmId) -> Option<Point> {
        match obj {
            OsmId::Node(n) => self.node_coords(n),
            OsmId::Way(w) => {
                let way = self.ways.get(w)?;
                way.nodes
                    .iter()
                    .map(|n| self.node_coords(n))
                    .collect::<Option<LineString>>()?
                    .centroid()
            }
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
                    "inner" | "" => None,
                    _ => unreachable!(),
                },
                osmpbfreader::OsmId::Relation(_) => {
                    assert_eq!(r.role, "subarea");
                    None
                }
            })
            .collect::<Vec<_>>();

        if ways.is_empty() {
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

pub fn read(reader: impl Read) -> anyhow::Result<Planet> {
    let mut pbf = OsmPbfReader::new(BufReader::with_capacity(1024 * 1024, reader));

    let mut planet = Planet::new();

    for res in pbf.par_iter() {
        planet.insert(res?);
    }

    Ok(planet)
}
