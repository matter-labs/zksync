use std::fmt::Debug;
use std::{str::FromStr, string::ToString};
use zksync_basic_types::TokenId;

pub fn vec_to_comma_list<T: ToString>(elems: Vec<T>) -> String {
    let strs: Vec<String> = elems.iter().map(|elem| (*elem).to_string()).collect();

    strs.join(",")
}

pub fn comma_list_to_vec<T: FromStr>(elems: String) -> Vec<T>
where
    <T as std::str::FromStr>::Err: Debug,
{
    elems
        .split(",")
        .map(|str| T::from_str(str).expect("Failed to deserialize stored item"))
        .collect()
}

// pub fn tokens_vec_to_str(token_ids: Vec<TokenId>) -> String {
//     let token_strings: Vec<String> = token_ids.iter().map(|&t| t.to_string()).collect();
//     token_strings.join(",")
// }

// pub fn tokens_str_to_vec(tokens: String) -> Vec<TokenId> {

// }

// pub fn hashes_vec_to_str(hashes: Vec<TxHash>) -> String {
//     let hashes_strings: Vec<String> = hashes.iter().map(|&h| h.toString()).collect();
//     hashes_strings.join(",")
// }

// pub fn tokens_str_to_vec(tokens: String) -> Vec<TokenId> {

// }
