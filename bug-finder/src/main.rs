use std::{cmp, fmt, process};

#[cfg(feature = "mmr_0_2")]
pub use mmr_0_2 as mmr;
#[cfg(feature = "mmr_0_3")]
pub use mmr_0_3 as mmr;
#[cfg(feature = "mmr_0_4")]
pub use mmr_0_4 as mmr;
#[cfg(feature = "mmr_0_5")]
pub use mmr_0_5 as mmr;

const BATCH_SIZE: u64 = 100_000;

#[derive(Eq, PartialEq, Clone, Default)]
struct NumberScope {
    start: u64,
    end: u64,
}

struct MergeNumberScope;

impl fmt::Debug for NumberScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.start == self.end {
            write!(f, "NumberScope(={})", self.start)
        } else {
            write!(f, "NumberScope({}, {})", self.start, self.end)
        }
    }
}

impl fmt::Display for NumberScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl fmt::Debug for MergeNumberScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MergeNumberScope")
    }
}

impl From<u64> for NumberScope {
    fn from(num: u64) -> Self {
        Self::new(num, num)
    }
}

impl NumberScope {
    fn new(start: u64, end: u64) -> Self {
        Self { start, end }
    }

    fn is_normalized(&self) -> bool {
        self.start <= self.end
    }
}

impl mmr::Merge for MergeNumberScope {
    type Item = NumberScope;
    #[cfg(any(feature = "mmr_0_2", feature = "mmr_0_3"))]
    fn merge(lhs: &Self::Item, rhs: &Self::Item) -> Self::Item {
        Self::Item {
            start: lhs.start,
            end: rhs.end,
        }
    }
    #[cfg(any(feature = "mmr_0_4", feature = "mmr_0_5"))]
    fn merge(lhs: &Self::Item, rhs: &Self::Item) -> mmr::Result<Self::Item> {
        Ok(Self::Item {
            start: lhs.start,
            end: rhs.end,
        })
    }
    #[cfg(all(
        any(feature = "mmr_0_4", feature = "mmr_0_5"),
        not(feature = "merge_left_to_right")
    ))]
    fn merge_peaks(lhs: &Self::Item, rhs: &Self::Item) -> mmr::Result<Self::Item> {
        log::trace!(
            "[{},{}] + [{},{}] -> [{},{}]",
            lhs.start,
            lhs.end,
            rhs.start,
            rhs.end,
            lhs.start,
            rhs.end,
        );
        Self::merge(lhs, rhs)
    }
    #[cfg(all(
        any(feature = "mmr_0_4", feature = "mmr_0_5"),
        feature = "merge_left_to_right"
    ))]
    fn merge_peaks(lhs: &Self::Item, rhs: &Self::Item) -> mmr::Result<Self::Item> {
        log::trace!(
            "[{},{}] + [{},{}] -> [{},{}]",
            rhs.start,
            rhs.end,
            lhs.start,
            lhs.end,
            rhs.start,
            lhs.end,
        );
        Self::merge(rhs, lhs)
    }
}

fn prepare_data(store: &mmr::util::MemStore<NumberScope>, leaf_count: u64) {
    let (mut mmr_size, leaf_next) = (0, 0);
    let mut mmr;
    let mut min = leaf_next;
    let mut max;
    loop {
        if min > leaf_count {
            break;
        }
        max = if min % BATCH_SIZE == 0 {
            cmp::min(min + BATCH_SIZE - 1, leaf_count)
        } else {
            cmp::min(((min / BATCH_SIZE) + 1) * BATCH_SIZE, leaf_count)
        };
        log::debug!("commit data for range [{},{}]", min, max);

        mmr = mmr::MMR::<_, MergeNumberScope, _>::new(mmr_size, store);
        for i in min..=max {
            mmr.push(NumberScope::from(i)).expect("push");
        }

        mmr_size = mmr.mmr_size();
        mmr.commit().expect("commit");

        min = max + 1;
    }
}

fn test_data(store: &mmr::util::MemStore<NumberScope>, leaf_count: u64, leaves: &[u64]) {
    let mmr_size = mmr::leaf_index_to_mmr_size(leaf_count - 1);
    let mmr = mmr::MMR::<_, MergeNumberScope, _>::new(mmr_size, store);
    let root = mmr.get_root().expect("get_root");
    log::debug!("root = {}", root);
    let leaves_data = leaves
        .iter()
        .map(|leaf| (mmr::leaf_index_to_pos(*leaf), NumberScope::from(*leaf)))
        .collect::<Vec<_>>();
    if !root.is_normalized() {
        log::debug!("root should be normalized, root: {}", root);
    }
    let proof = mmr
        .gen_proof(leaves_data.iter().map(|data| data.0).collect())
        .expect("gen_proof");
    for item in proof.proof_items() {
        if !item.is_normalized() {
            log::debug!("proof item should be normalized, item: {}", item);
        }
    }
    let result = proof.verify(root, leaves_data).expect("verify");
    if !result {
        log::error!("verify the proof: failed");
        process::exit(1);
    } else {
        log::debug!("verify the proof: passed");
    }
}

fn main() {
    env_logger::init();

    let store = mmr::util::MemStore::default();

    let leaf_count_max = 100u64;
    prepare_data(&store, leaf_count_max);

    for leaf_count in 1..=leaf_count_max {
        log::info!(">>> count = {}", leaf_count);
        let mut leaves_flag = 1u128;
        loop {
            let mut leaves_flags: Vec<_> = leaves_flag
                .to_le_bytes()
                .map(|val| {
                    let mut vec = vec![];
                    let mut mask = 0b0000_0001;
                    for _ in 0..8 {
                        vec.push(val & mask == mask);
                        mask <<= 1;
                    }
                    vec
                })
                .into_iter()
                .flatten()
                .collect();
            if leaves_flags[leaf_count as usize] {
                break;
            }
            leaves_flags.truncate(leaf_count as usize);
            log::trace!(">>> >>> leaves-flags = {:?}", leaves_flags);
            leaves_flag += 1;
            let leaves = leaves_flags
                .into_iter()
                .enumerate()
                .filter_map(|(idx, exists)| if exists { Some(idx as u64) } else { None })
                .collect::<Vec<_>>();
            log::trace!(">>> >>> leaves = {:?}", leaves);
            test_data(&store, leaf_count, &leaves);
        }
    }
}
