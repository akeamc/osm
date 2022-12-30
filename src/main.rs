use std::{
    mem::size_of,
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand};
use geo::{Contains, LineString, Point, Polygon};
use indicatif::{ProgressBar, ProgressStyle};
use osm::{
    planet::{self, nodes::Node, ways::Way},
    Record,
};
use osmpbfreader::{OsmId, RelationId};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use tracing::warn;

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
        input: String,
        /// Output file
        #[arg(long, short)]
        output: PathBuf,
    },
}

fn process_addresses(url: &str, output: &Path) -> anyhow::Result<()> {
    dbg!(size_of::<Node>());
    dbg!(size_of::<Way>());

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

    let planet = planet::read(&mut pb.wrap_read(reader))?;

    pb.finish();

    dbg!(
        planet.nodes.len(),
        planet.ways.len(),
        planet.relations.len()
    );

    eprintln!("forming polygons...");

    let mut csv = csv::Writer::from_path(output)?;

    let polygons = planet
        .relations
        .values()
        .filter_map(|rel| {
            if rel.tags.contains("boundary", "administrative") {
                if let Some(v) = planet.relation_rings(&rel.id) {
                    return Some(v.into_iter().filter_map(|nodes| {
                        let exterior = nodes
                            .into_iter()
                            .map(|n| planet.node_coords(&n))
                            .collect::<Option<LineString<_>>>()?;
                        let polygon = Polygon::new(exterior, vec![]);
                        Some((polygon, rel.id))
                    }));
                } else {
                    warn!("unable to form rings");
                };
            }

            None
        })
        .flatten()
        .collect::<Vec<_>>();

    eprintln!("formed {} polygons", polygons.len());

    let buildings = planet
        .nodes
        .par_iter()
        .map(|(k, _v)| OsmId::Node(*k))
        .chain(planet.ways.par_iter().map(|(k, _v)| OsmId::Way(*k)));

    let records = buildings
        .filter_map(|id| {
            let name = match id {
                OsmId::Node(n) => &planet.nodes.get(&n).unwrap().meta,
                OsmId::Way(w) => &planet.ways.get(&w).unwrap().meta,
                OsmId::Relation(_) => unreachable!(),
            }
            .name
            .as_ref()?;

            let point = planet.obj_coords(&id).unwrap();
            let mut rels = point_relations(&polygons, &point)
                .map(|(_p, rel)| planet.relations.get(rel).unwrap())
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

    tracing_subscriber::fmt::init();

    match command {
        Command::Address { input, output } => process_addresses(&input, &output),
    }
}
