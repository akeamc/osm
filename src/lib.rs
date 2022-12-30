use std::fmt::Display;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "address")]
pub mod planet;

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
pub enum OsmId {
    Node(i64),
    Way(i64),
    Relation(i64),
}

impl OsmId {
    pub fn as_i64(&self) -> i64 {
        match self {
            OsmId::Node(n) => *n,
            OsmId::Way(w) => *w,
            OsmId::Relation(r) => *r,
        }
    }
}

#[cfg(feature = "address")]
impl From<osmpbfreader::OsmId> for OsmId {
    fn from(o: osmpbfreader::OsmId) -> Self {
        match o {
            osmpbfreader::OsmId::Node(n) => Self::Node(n.0),
            osmpbfreader::OsmId::Way(w) => Self::Way(w.0),
            osmpbfreader::OsmId::Relation(r) => Self::Relation(r.0),
        }
    }
}

impl Display for OsmId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            match self {
                OsmId::Node(_) => "N",
                OsmId::Way(_) => "W",
                OsmId::Relation(_) => "R",
            },
            self.as_i64()
        )
    }
}

impl Serialize for OsmId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for OsmId {
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

        Ok(match discriminant {
            "N" => OsmId::Node(inner_id),
            "W" => OsmId::Way(inner_id),
            "R" => OsmId::Relation(inner_id),
            d => {
                return Err(de::Error::custom(format_args!(
                    "unrecognized discriminant `{d}`"
                )))
            }
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub name: String,
    pub osm_id: OsmId,
    #[serde(with = "json_str")]
    pub location: Vec<String>,
    pub latitude: f64,
    pub longitude: f64,
}
