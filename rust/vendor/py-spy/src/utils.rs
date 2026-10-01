use num_traits::{CheckedAdd, Zero};
use std::ops::Add;

pub fn is_subrange<T: Eq + Ord + Add + CheckedAdd + Zero>(
    start: T,
    size: T,
    sub_start: T,
    sub_size: T,
) -> bool {
    !size.is_zero()
        && !sub_size.is_zero()
        && start.checked_add(&size).is_some()
        && sub_start.checked_add(&sub_size).is_some()
        && sub_start >= start
        && sub_start + sub_size <= start + size
}

pub fn offset_of<T, M>(object: *const T, member: *const M) -> usize {
    member as usize - object as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_subrange() {
        assert!(is_subrange(
            0u64,
            0xffff_ffff_ffff_ffff,
            0,
            0xffff_ffff_ffff_ffff
        ));
        assert!(is_subrange(0, 1, 0, 1));
        assert!(is_subrange(0, 100, 0, 10));
        assert!(is_subrange(0, 100, 90, 10));

        assert!(!is_subrange(0, 0, 0, 0));
        assert!(!is_subrange(1, 0, 0, 0));
        assert!(!is_subrange(1, 0, 1, 0));
        assert!(!is_subrange(0, 0, 0, 1));
        assert!(!is_subrange(0, 0, 1, 0));
        assert!(!is_subrange(
            1u64,
            0xffff_ffff_ffff_ffff,
            0,
            0xffff_ffff_ffff_ffff
        ));
        assert!(!is_subrange(
            0u64,
            0xffff_ffff_ffff_ffff,
            1,
            0xffff_ffff_ffff_ffff
        ));
        assert!(!is_subrange(0, 10, 0, 11));
        assert!(!is_subrange(0, 10, 1, 10));
        assert!(!is_subrange(0, 10, 9, 2));
    }
}
