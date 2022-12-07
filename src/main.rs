use std::{collections::HashMap, fs::File};

use geo::{Contains, LineString, Point, Polygon};
use geojson::{Feature, FeatureCollection, GeoJson, PolygonType};
use osmpbfreader::{NodeId, OsmId, OsmObj, RelationId};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};

fn point_relations<'a>(
    polygons: &'a [(Polygon, RelationId)],
    point: &'a Point,
) -> impl Iterator<Item = &'a (Polygon, RelationId)> + 'a {
    polygons.iter().filter(|(p, _)| p.contains(point))
}

fn main() -> anyhow::Result<()> {
    // let osm = osm::read("faroe-islands.osm.pbf")?;
    // let osm = osm::read("albania.osm.pbf")?;
    let osm = osm::read("sweden.osm.pbf")?;

    let polygons = osm
        .relations
        .values()
        .filter_map(|rel| {
            if rel.tags.contains("boundary", "administrative") {
                if let Some(v) = osm.relation_rings(&rel.id) {
                    return Some(v.into_iter().filter_map(|nodes| {
                        let exterior = nodes
                            .into_iter()
                            .map(|n| osm.node_coords(&n))
                            .collect::<Option<LineString<_>>>()?;
                        let polygon = Polygon::new(exterior, vec![]);
                        Some((polygon, rel.id))
                    }));
                } else {
                    eprintln!("oh no!");
                };
            }

            None
        })
        .flatten()
        .collect::<Vec<_>>();

    eprintln!("{} polygons", polygons.len());

    let buildings = osm
        .nodes
        .par_iter()
        .map(|(k, v)| OsmId::Node(*k))
        .chain(osm.ways.par_iter().map(|(k, v)| OsmId::Way(*k)));

    // serde_json::to_writer(File::create("nodes.json")?, &buildings.collect::<Vec<_>>())?;
    // serde_json::to_writer(File::create("polys.json")?, &polygons)?;

    buildings.for_each(|id| {
        let tags = match id {
            OsmId::Node(n) => &osm.nodes.get(&n).unwrap().tags,
            OsmId::Way(w) => &osm.ways.get(&w).unwrap().tags,
            OsmId::Relation(_) => todo!(),
        };

        if !tags.contains_key("amenity") {
            return;
        }

        let point = osm.obj_coords(&id).unwrap();
        let mut rels = point_relations(&polygons, &point)
            .map(|(_p, rel)| osm.relations.get(rel).unwrap())
            .collect::<Vec<_>>();

        rels.sort_by_key(|rel| rel.tags.get("admin_level"));

        let location = rels
            .into_iter()
            .rev()
            .map(|rel| rel.tags.get("name").map(|t| t.as_str()).unwrap_or_default())
            .collect::<Vec<_>>()
            .join(", ");

        println!(
            "{:?} {:?}, {}",
            tags.get("name").map(|t| t.as_str()).unwrap_or_default(),
            id,
            location
        );
    });

    // let geojson = GeoJson::FeatureCollection(FeatureCollection {
    //     features: rings
    //         .into_iter()
    //         .map(|r| {
    //             let polygon = vec![r
    //                 .iter()
    //                 .map(|n| {
    //                     let p = osm.node_coords(n).unwrap();
    //                     vec![p.x(), p.y()]
    //                 })
    //                 .collect()];

    //             let geometry = geojson::Geometry::new(geojson::Value::Polygon(polygon));

    //             Feature {
    //                 geometry: Some(geometry),
    //                 ..Default::default()
    //             }
    //         })
    //         .collect(),
    //     bbox: None,
    //     foreign_members: None,
    // });

    // println!("{}", geojson);

    Ok(())
}
