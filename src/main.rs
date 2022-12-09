use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use geo::{Contains, LineString, Point, Polygon};
use osmpbfreader::{OsmId, RelationId};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use search::{osm, Record};

fn point_relations<'a>(
    polygons: &'a [(Polygon, RelationId)],
    point: &'a Point,
) -> impl Iterator<Item = &'a (Polygon, RelationId)> + 'a {
    polygons.iter().filter(|(p, _)| p.contains(point))
}

#[derive(Debug, Parser)]
struct Config {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Process addresses from an OSM file.
    Address {
        /// OSM planet file to parse (.osm.pbf).
        #[arg(long, short)]
        input: PathBuf,
        /// Output file
        #[arg(long, short)]
        output: PathBuf,
    },
}

fn process_addresses(input: &Path, output: &Path) -> anyhow::Result<()> {
    let osm = osm::read(input)?;

    let mut csv = csv::Writer::from_path(output)?;

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
                    eprintln!("unable to form rings");
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
        .map(|(k, _v)| OsmId::Node(*k))
        .chain(osm.ways.par_iter().map(|(k, _v)| OsmId::Way(*k)));

    let records = buildings
        .filter_map(|id| {
            let tags = match id {
                OsmId::Node(n) => &osm.nodes.get(&n).unwrap().tags,
                OsmId::Way(w) => &osm.ways.get(&w).unwrap().tags,
                OsmId::Relation(_) => unreachable!(),
            };

            if !tags.contains_key("amenity") {
                return None;
            }

            let name = tags.get("name")?;

            let point = osm.obj_coords(&id).unwrap();
            let mut rels = point_relations(&polygons, &point)
                .map(|(_p, rel)| osm.relations.get(rel).unwrap())
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

            Some(Record {
                name: name.to_string(),
                alt_name: tags.get("alt_name").map(|s| s.to_string()),
                operator: tags.get("operator").map(|s| s.to_string()),
                osm_id: id.into(),
                location,
                latitude: (point.y() * 1e7).round() / 1e7,
                longitude: (point.x() * 1e7).round() / 1e7,
            })
        })
        .collect::<Vec<_>>();

    eprintln!("writing csv");

    for record in records {
        csv.serialize(&record)?;
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let command = Config::parse().command;

    match command {
        Command::Address { input, output } => process_addresses(&input, &output),
    }
}
