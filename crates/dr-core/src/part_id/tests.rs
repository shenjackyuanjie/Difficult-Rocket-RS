
use super::*;

#[test]
fn allocation_keeps_normal_order_and_wraps_only_at_the_upper_boundary() {
    for (reserved, expected) in [
        (vec![], [1, 2, 3]),
        (vec![i64::MIN, -1, 0], [1, 2, 3]),
        (vec![1, 4], [5, 6, 7]),
        (vec![1, i64::MAX - 1], [i64::MAX, 2, 3]),
        (vec![2, 3, i64::MAX], [1, 4, 5]),
    ] {
        let mut allocator = PartIdAllocator::new(reserved);
        for id in expected {
            assert_eq!(allocator.allocate(), Some(id));
        }
    }
}
