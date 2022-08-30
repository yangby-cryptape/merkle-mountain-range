use std::{cmp, env, fmt, path::PathBuf, process};

#[cfg(feature = "mmr_0_2")]
pub use mmr_0_2 as mmr;
#[cfg(feature = "mmr_0_3")]
pub use mmr_0_3 as mmr;
#[cfg(feature = "mmr_0_4")]
pub use mmr_0_4 as mmr;
#[cfg(feature = "mmr_0_5")]
pub use mmr_0_5 as mmr;

mod database;

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

fn prepare_data(store: &database::Storage, leaf_count: u64) {
    let (mut mmr_size, leaf_next) = if let Some(leaf_index) = store.get_max() {
        let mmr_size = mmr::leaf_index_to_mmr_size(leaf_index);
        (mmr_size, leaf_index + 1)
    } else {
        (0, 0)
    };
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
        log::info!("commit data for range [{},{}]", min, max);

        mmr = mmr::MMR::<_, MergeNumberScope, _>::new(mmr_size, store);
        for i in min..=max {
            mmr.push(NumberScope::from(i)).expect("push");
        }

        mmr_size = mmr.mmr_size();
        mmr.commit().expect("commit");

        min = max + 1;
    }
}

fn test_data(store: &database::Storage, leaf_count: u64, leaves: &[u64]) {
    let mmr_size = mmr::leaf_index_to_mmr_size(leaf_count - 1);
    let mmr = mmr::MMR::<_, MergeNumberScope, _>::new(mmr_size, store);
    let root = mmr.get_root().expect("get_root");
    log::debug!("root = {}", root);
    let leaves_data = leaves
        .iter()
        .map(|leaf| (mmr::leaf_index_to_pos(*leaf), NumberScope::from(*leaf)))
        .collect::<Vec<_>>();
    if !root.is_normalized() {
        log::warn!("root should be normalized, root: {}", root);
    }
    let proof = mmr
        .gen_proof(leaves_data.iter().map(|data| data.0).collect())
        .expect("gen_proof");
    for item in proof.proof_items() {
        if !item.is_normalized() {
            log::warn!("proof item should be normalized, item: {}", item);
        }
    }
    let result = proof.verify(root, leaves_data).expect("verify");
    if !result {
        log::error!("verify the proof: failed");
    } else {
        log::info!("verify the proof: passed");
    }
}

fn main() {
    env_logger::init();

    let mut args = env::args();
    let argv = args.len();
    let progname = args.next().unwrap();
    let db_path = if argv == 2 {
        let db_path = args.next().unwrap();
        PathBuf::from(db_path)
    } else {
        eprintln!("{} <DB_FILE_PATH>", progname);
        process::exit(1);
    };

    let store = database::Storage::new(db_path);

    let leaf_count = 6600000u64;
    prepare_data(&store, leaf_count);

    let leaf_count = 6525231u64;
    let leaves = vec![
        10831u64, 336173, 445650, 613398, 658314, 1424918, 1893395, 1986753, 2088123, 2107991,
        2186763, 2198865, 2308158, 2360180, 2735886, 3008095, 3024157, 3197172, 3447883, 3468607,
        3581739, 3662312, 3762812, 3837880, 3855444, 3937962, 4001151, 4040852, 4047540, 4093434,
        4213840, 4224947, 4435092, 4475938, 4532206, 4565515, 4640728, 4716574, 4739211, 4804833,
        4841132, 4862306, 5021305, 5058646, 5079878, 5107810, 5131910, 5148118, 5184374, 5240371,
        5240554, 5242601, 5285816, 5431049, 5491750, 5518275, 5590677, 5617502, 5618345, 5626969,
        5657470, 5660505, 5721416, 5725329, 5777794, 5791511, 5793592, 5814929, 5816997, 5826092,
        5836423, 5852205, 5856842, 5859852, 5871282, 5916048, 5916768, 5934740, 5936211, 5943004,
        5950867, 5959984, 5976814, 5991261, 6004768, 6012617, 6018500, 6027644, 6028689, 6054772,
        6058580, 6067642, 6071896, 6072374, 6078601, 6086562, 6115268, 6119216, 6122465, 6127800,
        6133008, 6135687, 6137171, 6139921, 6151622, 6157952, 6159641, 6164396, 6180295, 6183008,
        6183714, 6188136, 6189493, 6197827, 6203217, 6206971, 6226446, 6233674, 6252698, 6256175,
        6256932, 6260495, 6260645, 6260787, 6260966, 6264982, 6269407, 6272990, 6274139, 6276060,
        6278168, 6280755, 6295977, 6297685, 6301654, 6306476, 6308995, 6309755, 6312566, 6315166,
        6336194, 6336393, 6338582, 6339250, 6339300, 6342385, 6344375, 6351265, 6353993, 6355225,
        6361951, 6363947, 6371820, 6376213, 6380308, 6381675, 6383992, 6386418, 6387821, 6391581,
        6396471, 6398905, 6408918, 6410868, 6413936, 6416926, 6417170, 6418155, 6420079, 6420448,
        6421896, 6424955, 6425467, 6430388, 6431467, 6433248, 6433581, 6434017, 6434971, 6437899,
        6437949, 6438976, 6442014, 6444133, 6446597, 6447663, 6447882, 6449487, 6449876, 6449961,
        6451213, 6454549, 6456102, 6458543, 6461391, 6463231, 6464078, 6465811, 6468428, 6468864,
        6469002, 6471197, 6472305, 6472545, 6480117, 6481950, 6485330, 6485345, 6486063, 6486248,
        6486429, 6486786, 6487704, 6488426, 6489539, 6490888, 6491724, 6491877, 6493390, 6493971,
        6493984, 6495373, 6495858, 6496162, 6496810, 6497628, 6497668, 6498259, 6498644, 6499086,
        6499829, 6499904, 6499943, 6500966, 6501653, 6502470, 6502809, 6503245, 6503487, 6504495,
        6504800, 6505126, 6505186, 6505897, 6505915, 6506786, 6507999, 6508894, 6509159, 6510034,
        6510567, 6510677, 6511080, 6511365, 6511504, 6511622, 6512576, 6513740, 6514164, 6514317,
        6514640, 6515202, 6515321, 6515358, 6515402, 6515561, 6515581, 6515821, 6516432, 6516514,
        6516680, 6516831, 6517093, 6517255, 6517837, 6518134, 6518245, 6518462, 6518825, 6518851,
        6518916, 6519009, 6519203, 6519257, 6519387, 6519504, 6519609, 6519619, 6519926, 6520111,
        6520389, 6520658, 6520769, 6520874, 6520991, 6520995, 6521062, 6521126, 6521164, 6521301,
        6521459, 6521492, 6521646, 6522014, 6522352, 6522362, 6522377, 6522651, 6522725, 6522832,
        6522836, 6522852, 6522875, 6522951, 6522976, 6523097, 6523209, 6523294, 6523365, 6523409,
        6523422, 6523469, 6523525, 6523547, 6523573, 6523591, 6523658, 6523717, 6523741, 6523753,
        6523792, 6523804, 6523890, 6523952, 6523955, 6523958, 6523962, 6524016, 6524027, 6524036,
        6524082, 6524132, 6524136, 6524143, 6524159, 6524212, 6524222, 6524258, 6524261, 6524284,
        6524311, 6524320, 6524324, 6524335, 6524353, 6524354, 6524366, 6524401, 6524443, 6524481,
        6524488, 6524500, 6524504, 6524558, 6524579, 6524642, 6524653, 6524673, 6524698, 6524721,
        6524743, 6524766, 6524784, 6524790, 6524821, 6524842, 6524852, 6524870, 6524871, 6524897,
        6524902, 6524904, 6524907, 6524908, 6524911, 6524923, 6524924, 6524931, 6524934, 6524968,
        6524973, 6524974, 6524982, 6524983, 6524995, 6525015, 6525019, 6525027, 6525031, 6525039,
        6525044, 6525051, 6525054, 6525058, 6525066, 6525068, 6525069, 6525080, 6525085, 6525088,
        6525095, 6525097, 6525099, 6525100, 6525104, 6525110, 6525115, 6525118, 6525120, 6525122,
        6525127, 6525131, 6525131, 6525132, 6525133, 6525134, 6525135, 6525136, 6525137, 6525138,
        6525139, 6525140, 6525141, 6525142, 6525143, 6525144, 6525145, 6525146, 6525147, 6525148,
        6525149, 6525150, 6525151, 6525152, 6525153, 6525154, 6525155, 6525156, 6525157, 6525158,
        6525159, 6525160, 6525161, 6525162, 6525163, 6525164, 6525165, 6525166, 6525167, 6525168,
        6525169, 6525170, 6525171, 6525172, 6525173, 6525174, 6525175, 6525176, 6525177, 6525178,
        6525179, 6525180, 6525181, 6525182, 6525183, 6525184, 6525185, 6525186, 6525187, 6525188,
        6525189, 6525190, 6525191, 6525192, 6525193, 6525194, 6525195, 6525196, 6525197, 6525198,
        6525199, 6525200, 6525201, 6525202, 6525203, 6525204, 6525205, 6525206, 6525207, 6525208,
        6525209, 6525210, 6525211, 6525212, 6525213, 6525214, 6525215, 6525216, 6525217, 6525218,
        6525219, 6525220, 6525221, 6525222, 6525223, 6525224, 6525225, 6525226, 6525227, 6525228,
        6525229, 6525230,
    ];
    test_data(&store, leaf_count, &leaves);
}
