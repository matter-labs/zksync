use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use super::{from_hex, to_hex};
use super::node::{Fr};

pub fn optional_fr_ser<S: Serializer>(
    value: &Option<Fr>,
    ser: S,
) -> Result<S::Ok, S::Error> {
    let v = value.map(|a| to_hex(&a));

    Option::serialize(&v, ser)
}

pub fn optional_fr_de<'de, D>(deserializer: D) -> Result<Option<Fr>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;

    if let Some(a) = s {
        let v = from_hex(&a).map_err(|e| de::Error::custom(e))?;
        Ok(Some(v))
    } else {
        Ok(None)
    }
}

pub fn fr_ser<S: Serializer>(
    value: &Fr,
    ser: S,
) -> Result<S::Ok, S::Error> {
    let v = to_hex(value);

    String::serialize(&v, ser)
}

pub fn fr_de<'de, D>(deserializer: D) -> Result<Fr, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = String::deserialize(deserializer)?;

    let v = from_hex(&s).map_err(|e| de::Error::custom(e))?;
    Ok(v)
}

pub fn vec_optional_fr_ser<S: Serializer>(
    operations: &[Option<Fr>],
    ser: S,
) -> Result<S::Ok, S::Error> {
    let mut res = Vec::with_capacity(operations.len());
    for value in operations.iter() {
        let v = value.map(|a| to_hex(&a));
        res.push(v);
    }
    Vec::serialize(&res, ser)
}

pub fn vec_optional_fr_de<'de, D>(deserializer: D) -> Result<Vec<Option<Fr>>, D::Error>
where
    D: Deserializer<'de>,
{
    let str_vec: Vec<Option<String>> = Vec::deserialize(deserializer)?;
    let mut res = Vec::with_capacity(str_vec.len());
    for s in str_vec.into_iter() {
        if let Some(a) = s {
            let v = from_hex(&a).map_err(|e| de::Error::custom(e))?;
            res.push(Some(v));
        } else {
            res.push(None);
        }
    }
    Ok(res)
}