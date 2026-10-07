// What rustc does with Generics::param_def_id_to_index across sessions.
use rustc_hash::FxHashMap;

// DefId's Hash impl on 64-bit targets: (krate << 32) | index, as one u64.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct DefId {
    index: u32,
    krate: u32,
}
impl std::hash::Hash for DefId {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        (((self.krate as u64) << 32) | self.index as u64).hash(h)
    }
}

fn order(m: &FxHashMap<DefId, u32>) -> Vec<(u32, u32)> {
    m.iter().map(|(k, v)| (k.index, *v)).collect()
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("buckets") {
        return buckets();
    }
    let first: Vec<u32> = std::env::args()
        .skip(1)
        .map(|a| a.parse().unwrap())
        .collect();
    // generics_of: own_params.iter().map(|p| (p.def_id, p.index)).collect()
    let built: FxHashMap<DefId, u32> = first
        .iter()
        .enumerate()
        .map(|(i, &d)| (DefId { index: d, krate: 0 }, i as u32))
        .collect();
    println!("clean session, built and encoded:    {:?}", order(&built));
    // Encodable writes iteration order; Decodable collects in that order.
    let mut map = built;
    for session in 2..=4 {
        let decoded: FxHashMap<DefId, u32> = order(&map)
            .into_iter()
            .map(|(d, i)| (DefId { index: d, krate: 0 }, i))
            .collect();
        println!(
            "session {session}, decoded from the cache: {:?}",
            order(&decoded)
        );
        map = decoded;
    }
}

#[allow(dead_code)]
pub fn buckets() {
    use std::hash::BuildHasher;
    for i in [12u32, 13, 14] {
        let h = rustc_hash::FxBuildHasher.hash_one(DefId { index: i, krate: 0 });
        println!(
            "DefIndex {i}: hash {h:#018x}, bucket in a 4-bucket table {}",
            h & 3
        );
    }
}
