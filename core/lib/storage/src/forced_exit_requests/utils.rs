use zksync_basic_types::TokenId;

pub fn tokens_vec_to_str(token_ids: Vec<TokenId>) -> String {
    let token_strings: Vec<String> = token_ids.iter().map(|t| (*t).to_string()).collect();
    token_strings.join(",")
}
