#![allow(dead_code)]
#![cfg_attr(feature = "nightly", feature(test))]

#[cfg(all(feature = "nightly", test))]
extern crate test;

const COMPRESSED_LZ4: &[u8] = include_bytes!("../tests/fixtures/compressed_lz4.bag");

#[cfg(all(feature = "nightly", test))]
#[cfg(test)]
mod tests {
    use super::*;
    use frost::{query::Query, Bag};
    use test::Bencher;

    #[bench]
    fn bench_from_bytes(b: &mut Bencher) {
        b.iter(|| {
            for _i in 0..1000 {
                let mut _bag = Bag::from_bytes(COMPRESSED_LZ4).unwrap();
            }
        });
    }

    #[bench]
    fn bench_query_all(b: &mut Bencher) {
        let mut bag = Bag::from_bytes(COMPRESSED_LZ4).unwrap();
        let query = Query::all();

        b.iter(|| {
            for _i in 0..1000 {
                let _count = bag.read_messages(&query).unwrap().count();
            }
        });
    }
}
