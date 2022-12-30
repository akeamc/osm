use std::fmt::Display;

use geo::Point;
use osmpbfreader::{NodeId, OsmId, RelationId, WayId};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

pub mod planet;
#[cfg(feature = "search")]
pub mod search;

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

#[derive(Debug)]
pub struct OsmIdWrapper(pub OsmId);

impl From<OsmId> for OsmIdWrapper {
    fn from(o: OsmId) -> Self {
        Self(o)
    }
}

impl Display for OsmIdWrapper {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            match self.0 {
                OsmId::Node(_) => "N",
                OsmId::Way(_) => "W",
                OsmId::Relation(_) => "R",
            },
            self.0.inner_id()
        )
    }
}

impl Serialize for OsmIdWrapper {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OsmIdWrapper {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let discriminant = s
            .get(0..1)
            .ok_or_else(|| de::Error::custom("missing osm id type"))?;
        let inner_id = s
            .get(1..)
            .ok_or_else(|| de::Error::custom("missing id"))?
            .parse::<i64>()
            .map_err(de::Error::custom)?;

        Ok(Self(match discriminant {
            "N" => OsmId::Node(NodeId(inner_id)),
            "W" => OsmId::Way(WayId(inner_id)),
            "R" => OsmId::Relation(RelationId(inner_id)),
            d => {
                return Err(de::Error::custom(format_args!(
                    "unrecognized discriminant `{d}`"
                )))
            }
        }))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub name: String,
    pub osm_id: OsmIdWrapper,
    #[serde(with = "json_str")]
    pub location: Vec<String>,
    pub latitude: f64,
    pub longitude: f64,
}

impl Record {
    #[cfg(feature = "search")]
    fn into_milli_document(self) -> milli::Object {
        use serde_json::{Map, Value};

        let Self {
            name,
            osm_id,
            location,
            latitude,
            longitude,
        } = self;

        let mut map = Map::new();

        map.insert("id".to_owned(), Value::String(osm_id.to_string()));

        map.insert("name".to_owned(), Value::String(name));

        map.insert(
            "location".to_owned(),
            Value::Array(location.into_iter().map(Value::String).collect()),
        );

        let geo = {
            let mut coordinates = serde_json::Map::new();

            coordinates.insert(
                "lat".to_owned(),
                Value::Number(serde_json::Number::from_f64(latitude).unwrap()),
            );
            coordinates.insert(
                "lon".to_owned(),
                Value::Number(serde_json::Number::from_f64(longitude).unwrap()),
            );

            coordinates
        };

        map.insert("_geo".to_owned(), Value::Object(geo));

        map
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MilliGeo {
    lat: f64,
    lon: f64,
}

impl From<MilliGeo> for Point {
    fn from(MilliGeo { lat, lon }: MilliGeo) -> Self {
        Point::new(lon, lat)
    }
}
