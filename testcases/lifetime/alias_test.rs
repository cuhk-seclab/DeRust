use std::collections::HashSet;

fn main() {
    let alias1 = vec![15, 26, 17, 16];
    let alias2 = vec![15, 26];
    let mut new_alias = Vec::new();
    let mut seen = HashSet::new();
    let mut hash_alias = Vec::new();

    let alias_chain = alias1
        .iter()
        .chain(alias2.iter())
        .cloned()
        .collect::<Vec<_>>();
    let mut alias_extend = Vec::new();
    alias_extend.extend(alias1.clone());
    alias_extend.extend(alias2.clone());
    println!("alias_chain: {:?}", alias_chain);
    println!("alias_extend: {:?}", alias_extend);

    for &alias in alias_extend.iter() {
        if seen.insert(alias) {
            new_alias.push(alias);
        }
    }

    hash_alias = alias_extend
        .clone()
        .into_iter()
        .collect::<HashSet<usize>>()
        .into_iter()
        .collect();
    println!("seen: {:?}", seen);
    println!("new_alias: {:?}", new_alias);
    println!("hash_alias: {:?}", hash_alias);
}
