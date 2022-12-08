use std::{io::Cursor, time::Instant};

use fuzzy_geo::Record;
use milli::{
    documents::{DocumentsBatchBuilder, DocumentsBatchReader},
    heed::EnvOpenOptions,
    update::{IndexDocuments, IndexDocumentsConfig, IndexerConfig},
    Index, Search,
};
use serde_json::Value;

fn main() -> anyhow::Result<()> {
    let path = tempfile::tempdir().unwrap();
    let mut options = EnvOpenOptions::new();
    options.map_size(100 * 1024 * 1024); // 100 MiB
    let index = Index::new(options, &path).unwrap();

    let mut wtxn = index.write_txn().unwrap();

    let mut builder = DocumentsBatchBuilder::new(Vec::new());

    for res in csv::Reader::from_path("names.csv")?.into_deserialize() {
        let Record {
            name,
            alt_name,
            operator,
            osm_id,
            location,
            latitude,
            longitude,
        } = res?;

        let mut map = serde_json::Map::new();
        map.insert(
            "id".to_owned(),
            Value::String(osm_id.inner_id().to_string()),
        );
        map.insert("name".to_owned(), Value::String(name));
        map.insert(
            "location".to_owned(),
            Value::Array(location.into_iter().map(Value::String).collect()),
        );

        let mut geo = serde_json::Map::new();
        geo.insert(
            "lat".to_owned(),
            Value::Number(serde_json::Number::from_f64(latitude).unwrap()),
        );
        geo.insert(
            "lon".to_owned(),
            Value::Number(serde_json::Number::from_f64(longitude).unwrap()),
        );

        map.insert("_geo".to_owned(), Value::Object(geo));

        builder.append_json_object(&map)?;
    }

    let file = Cursor::new(builder.into_inner()?);

    let config = IndexerConfig::default();
    let indexing_config = IndexDocumentsConfig::default();
    let builder = IndexDocuments::new(
        &mut wtxn,
        &index,
        &config,
        indexing_config,
        |_| (),
        || false,
    )
    .unwrap();
    let (builder, res) = builder.add_documents(DocumentsBatchReader::from_reader(file)?)?;
    res?;
    builder.execute()?;
    wtxn.commit().unwrap();

    let start = Instant::now();

    // You can search in the index now!
    let rtxn = index.read_txn().unwrap();
    let mut search = Search::new(&rtxn, &index);
    search.query("Södra Latins gymnasium, Fraiche Catering");
    search.limit(10);

    let result = search.execute().unwrap();

    let elapsed = start.elapsed();

    let field_ids_map = index.fields_ids_map(&rtxn).unwrap();

    for (_id, obkv) in index.documents(&rtxn, result.documents_ids)? {
        let mut document = serde_json::Map::new();

        // recreate the original json
        for (key, value) in obkv.iter() {
            let value = serde_json::from_slice(value)?;
            let key = field_ids_map
                .name(key)
                .expect("Missing field name")
                .to_string();

            document.insert(key, value);
        }

        dbg!(document);
    }

    eprintln!("search took {elapsed:.02?}");

    Ok(())
}
