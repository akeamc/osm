use std::path::{Path, PathBuf};

use clap::Parser;
use geo::{Centroid, Contains, LineString, MultiPolygon, Polygon};
use indicatif::{ProgressBar, ProgressStyle};
use osm::BUILDING_LEVEL;
use osmpbfreader::OsmId;
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[derive(Debug, Parser)]
struct Config {
    /// URL of OSM planet file to parse (.osm.pbf).
    #[arg(long, short)]
    input: String,
    /// Output file.
    #[arg(long, short)]
    output: PathBuf,
}

fn process_addresses(url: &str, output: &Path) -> anyhow::Result<()> {
    let response = ureq::get(url).call()?;
    let len = response.header("content-length").unwrap().parse().unwrap();
    let reader = response.into_reader();

    eprintln!("downloading and parsing planet file...");
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
        )
        .unwrap()
        .progress_chars("#>-"),
    );

    let planet = osm::planet::read(&mut pb.wrap_read(reader))?;

    pb.finish();

    eprintln!("- {} nodes", planet.nodes.len());
    eprintln!("- {} ways", planet.ways.len());
    eprintln!("- {} relations", planet.relations.len());

    eprintln!("forming polygons...");

    let mut csv = csv::Writer::from_path(output)?;

    let polygons = planet
        .relations
        .values()
        .filter_map(|rel| {
            if rel.tags.contains("boundary", "administrative") {
                if let Some(v) = planet.relation_rings(&rel.id) {
                    return Some((
                        rel.id,
                        v.into_iter()
                            .filter_map(|nodes| {
                                let exterior = nodes
                                    .into_iter()
                                    .map(|n| planet.node_coords(&n))
                                    .collect::<Option<LineString<_>>>()?;
                                let polygon = Polygon::new(exterior, vec![]);
                                Some(polygon)
                            })
                            .collect::<Vec<_>>(),
                    ));
                } else {
                    #[cfg(feature = "tracing")]
                    tracing::warn!(?rel.id, "unable to form rings");
                };
            }

            None
        })
        .collect::<Vec<_>>();

    eprintln!("formed {} polygons", polygons.len());

    let buildings = planet
        .nodes
        .par_iter()
        .map(|(k, _v)| OsmId::Node(*k))
        .chain(planet.ways.par_iter().map(|(k, _v)| OsmId::Way(*k)));

    let buildings = buildings
        .filter_map(|id| {
            let name = match id {
                OsmId::Node(n) => &planet.nodes.get(&n).unwrap().meta,
                OsmId::Way(w) => &planet.ways.get(&w).unwrap().meta,
                OsmId::Relation(_) => unreachable!(),
            }
            .name
            .as_ref()?;

            let point = planet.obj_coords(&id).unwrap();
            let mut rels = polygons
                .iter()
                .flat_map(|(r, v)| v.iter().map(move |p| (r, p)))
                .filter(|(_rel, p)| p.contains(&point))
                .map(|(rel, _p)| planet.relations.get(rel).unwrap())
                .filter_map(|rel| {
                    let name = rel.tags.get("name")?.as_str();
                    let admin_level: u8 = rel.tags.get("admin_level")?.parse().unwrap();

                    Some((name, admin_level))
                })
                .collect::<Vec<_>>();

            rels.sort_by_key(|(_name, lvl)| *lvl);

            let location = rels
                .into_iter()
                .rev()
                .map(|(name, _lvl)| name.to_string())
                .collect::<Vec<_>>();

            Some(osm::Record {
                name: name.to_string(),
                osm_id: id.into(),
                location,
                latitude: (point.y() * 1e7).round() / 1e7,
                longitude: (point.x() * 1e7).round() / 1e7,
                level: BUILDING_LEVEL,
            })
        })
        .collect::<Vec<_>>();

    eprintln!("writing csv");

    for record in buildings {
        csv.serialize(&record)?;
    }

    let relations = polygons
        .into_par_iter()
        .filter_map(|(id, polygons)| {
            let rel = planet.relations.get(&id).unwrap();
            let name = rel.tags.get("name")?.as_str();
            let admin_level: u8 = rel.tags.get("admin_level")?.parse().unwrap();
            let center = rel
                .refs
                .iter()
                .find(|r| r.role == "admin_centre")
                .and_then(|r| planet.obj_coords(&r.member))
                .or(MultiPolygon::new(polygons).centroid())?;

            assert!(admin_level < BUILDING_LEVEL);

            Some(osm::Record {
                name: name.to_string(),
                osm_id: osmpbfreader::OsmId::Relation(id).into(),
                location: vec![],
                latitude: (center.y() * 1e7).round() / 1e7,
                longitude: (center.x() * 1e7).round() / 1e7,
                level: admin_level,
            })
        })
        .collect::<Vec<_>>();

    for record in relations {
        csv.serialize(&record)?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let Config { input, output } = Config::parse();

    tracing_subscriber::fmt::init();

    process_addresses(&input, &output)
}
