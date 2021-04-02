use std::fmt::Debug;
use std::{str::FromStr, string::ToString};

pub fn vec_to_comma_list<T: ToString>(elems: Vec<T>) -> String {
    let strs: Vec<String> = elems.iter().map(|elem| (*elem).to_string()).collect();

    strs.join(",")
}

pub fn comma_list_to_vec<T: FromStr>(elems: String) -> Vec<T>
where
    <T as std::str::FromStr>::Err: Debug,
{
    elems
        .split(',')
        .map(|str| T::from_str(str).expect("Failed to deserialize stored item"))
        .collect()
}
