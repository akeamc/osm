use geojson::{Feature, FeatureCollection, GeoJson};
use osmpbfreader::NodeId;

fn main() -> anyhow::Result<()> {
    let osm = osm::read("sweden.osm.pbf")?;

    let mut rings: Vec<Vec<NodeId>> = Vec::new();

    for rel in osm.relations.values() {
        if rel.tags.contains("boundary", "administrative") {
            if let Some(r) = osm.relation_polygon(&rel.id) {
                rings.extend_from_slice(&r[..]);
            };
        }
    }

    let geojson = GeoJson::FeatureCollection(FeatureCollection {
        features: rings
            .into_iter()
            .map(|r| {
                let geometry = geojson::Geometry::new(geojson::Value::LineString(
                    r.iter()
                        .map(|n| {
                            let p = osm.node_coords(n).unwrap();
                            vec![p.x(), p.y()]
                        })
                        .collect(),
                ));

                Feature {
                    geometry: Some(geometry),
                    ..Default::default()
                }
            })
            .collect(),
        bbox: None,
        foreign_members: None,
    });

    println!("{}", geojson);

    Ok(())
}
