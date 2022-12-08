use std::{
    collections::{BTreeSet, HashMap},
    io::Read,
};

use anyhow::Result;
use geo::Point;
use osmpbfreader::OsmId;
use serde::{Deserialize, Serialize};

mod json_str {
    use serde::{
        de::{self, DeserializeOwned},
        ser, Deserialize, Deserializer, Serialize, Serializer,
    };

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        serde_json::to_string(value)
            .map_err(ser::Error::custom)?
            .serialize(serializer)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: DeserializeOwned,
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        serde_json::from_str(&s).map_err(de::Error::custom)
    }
}

mod osm_serde {
    use osmpbfreader::OsmId;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(value: &OsmId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value.inner_id().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<OsmId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = i64::deserialize(deserializer)?;
        Ok(OsmId::Node(osmpbfreader::NodeId(inner)))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub name: String,
    pub alt_name: Option<String>,
    pub operator: Option<String>,
    #[serde(with = "osm_serde")]
    pub osm_id: OsmId,
    #[serde(with = "json_str")]
    pub location: Vec<String>,
    pub latitude: f64,
    pub longitude: f64,
}

impl Record {
    fn words<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        self.name
            .split_whitespace()
            .chain(self.location.iter().flat_map(|s| s.split_whitespace()))
    }
}

pub struct Builder {
    points: HashMap<OsmId, Point>,
    index: HashMap<Vec<u8>, BTreeSet<OsmId>>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            points: Default::default(),
            index: Default::default(),
        }
    }

    fn insert_record(&mut self, record: Record) {
        let p = Point::new(record.longitude, record.latitude);
        self.points.insert(record.osm_id, p);

        for word in record.words() {
            self.index
                .entry(word.as_bytes().to_vec())
                .or_default()
                .insert(record.osm_id);
        }
    }

    pub fn from_reader<R>(reader: R) -> Result<Self>
    where
        R: Read,
    {
        let mut builder = Self::new();

        for res in csv::Reader::from_reader(reader).into_deserialize::<Record>() {
            builder.insert_record(res?);
        }

        Ok(builder)
    }

    pub fn build(self) -> Table {
        let Self { points, index } = self;

        Table { points, index }
    }
}

pub struct Table {
    points: HashMap<OsmId, Point>,
    index: HashMap<Vec<u8>, BTreeSet<OsmId>>,
}
