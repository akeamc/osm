use std::{
    io::{Cursor, Read, Seek, Write},
    path::Path,
};

use geo::Point;
use milli::{
    documents::{DocumentsBatchBuilder, DocumentsBatchReader},
    heed::EnvOpenOptions,
    update::{IndexDocuments, IndexDocumentsConfig, IndexerConfig},
    FieldsIdsMap, Index,
};
use osmpbfreader::OsmId;

use crate::{MilliGeo, OsmIdWrapper, Record};

pub struct IndexBuilder {
    inner: Index,
}

impl IndexBuilder {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut options = EnvOpenOptions::new();
        options.map_size(51 * 1024 * 1024); // 51 MiB (just enough)

        Ok(Self {
            inner: milli::Index::new(options, path)?,
        })
    }

    pub fn with_csv_and_buffer(
        self,
        csv: impl Read,
        buffer: impl Read + Write + Seek,
    ) -> anyhow::Result<Self> {
        let mut wtxn = self.inner.write_txn()?;
        let mut builder = DocumentsBatchBuilder::new(buffer);

        for res in csv::Reader::from_reader(csv).into_deserialize::<Record>() {
            builder.append_json_object(&res?.into_milli_document())?;
        }

        let buffer = builder.into_inner()?;
        let config = IndexerConfig::default();
        let indexing_config = IndexDocumentsConfig::default();
        let builder = IndexDocuments::new(
            &mut wtxn,
            &self.inner,
            &config,
            indexing_config,
            |_| (),
            || false,
        )?;

        let (builder, res) = builder.add_documents(DocumentsBatchReader::from_reader(buffer)?)?;
        res?;
        builder.execute()?;
        wtxn.commit()?;

        Ok(self)
    }

    pub fn with_csv(self, csv: impl Read) -> anyhow::Result<Self> {
        self.with_csv_and_buffer(csv, Cursor::new(Vec::new()))
    }

    pub fn build(self) -> Index {
        self.inner
    }
}

pub struct ParsedRecord {
    pub name: String,
    pub id: OsmId,
    pub coordinates: Point,
}

pub fn parse_obkv(
    fields_ids_map: &FieldsIdsMap,
    obkv: obkv::KvReader<'_, u16>,
) -> anyhow::Result<ParsedRecord> {
    let mut name = None;
    let mut id = None;
    let mut coordinates: Option<Point> = None;

    for (key, value) in obkv.iter() {
        match fields_ids_map.name(key).expect("missing field name") {
            "name" => name = Some(serde_json::from_slice::<String>(value)?),
            "id" => id = Some(serde_json::from_slice::<OsmIdWrapper>(value)?.0),
            "_geo" => coordinates = Some(serde_json::from_slice::<MilliGeo>(value)?.into()),
            _ => continue,
        }
    }

    Ok(ParsedRecord {
        name: name.unwrap(),
        id: id.unwrap(),
        coordinates: coordinates.unwrap(),
    })
}
