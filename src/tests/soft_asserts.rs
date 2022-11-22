use alloc::format;
use alloc::string::String;
use core::fmt::{Debug, Display, LowerHex};
use core::mem::transmute;
use core::ops::{Add, RangeInclusive, Sub};
use crate::math::vector::Vector;

/// Tests if `v1 == v2`.
pub fn soft_assert_eq<T: Debug + PartialEq>(v1: T, v2: T, help: &str) -> Result<(), String> {
    if v1 == v2 {
        Ok(())
    } else {
        Err(format!("a == b expected, but a={:#x?} b={:#x?}. {}", v1, v2, help))
    }
}

/// Tests if `v1 == v2` with a delta.
pub fn soft_assert_eq_with_epsilon<T: Copy + Clone + Debug + PartialOrd + Add<Output = T> + Sub<Output = T>>(epsilon: T, actual: T, expected: T, help: &str) -> Result<(), String> {
    if actual >= expected - epsilon && actual <= expected + epsilon {
        Ok(())
    } else {
        Err(format!("Actual: {:?} but expected: {:?} (+/- {:?}). {}", actual, expected, epsilon, help))
    }
}

/// Inlined test of whether `v1 == v2`. Similar to [`soft_assert_eq`] but the help message on failure
/// is provided via a closure/fn instead of a `&str`.
#[inline(always)]
pub fn soft_assert_eq2<T: Debug + PartialEq + Eq, H: FnOnce() -> String>(v1: T, v2: T, help: H) -> Result<(), String> {
    if v1 == v2 {
        Ok(())
    } else {
        Err(format!("a == b expected, but a={:#x?} b={:#x?}. {}", v1, v2, help()))
    }
}

/// Tests if `v1 == v2` but print decimal.
pub fn soft_assert_eq_decimal<T: Debug + PartialEq>(actual: T, expected: T, help: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("a == b expected, but: Actual: {:#?}, expected {:#?}. {}", actual, expected, help))
    }
}

/// Inlined test of whether [vectors](Vector) `v1 == v2`, Equivalent to [`soft_assert_eq2`] but prints
/// a more readable error message on failure.
#[inline(always)]
pub fn soft_assert_eq_vector<H: FnOnce() -> String>(actual: Vector, expected: Vector, help: H) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        // Doing typography with spaces...ugly
        Err(format!("a == b expected, but (hex):\nActual:     {:04x?}\nExpected: {:04x?}\n{}", actual, expected, help()))
    }
}

/// Inlined test of whether 2D arrays `v1 == v2`, Equivalent to [`soft_assert_eq2`] but prints
/// a more readable error message on failure.
#[inline(always)]
pub fn soft_assert_eq_2d_array<H: FnOnce() -> String, T: Debug + PartialEq + Eq, const X: usize, const Y: usize>(actual: [[T; X]; Y], expected: [[T; X]; Y], help: H) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        // Doing typography with spaces...ugly
        fn format<T: Debug + PartialEq + Eq, const X: usize, const Y: usize>(data: [[T; X]; Y]) -> String {
            let mut result = String::new();
            for row_index in 0..Y {
                if row_index != 0 {
                    result.push('\n');
                }
                result.push_str(format!("{:04X?}", data[row_index]).as_str());
            }
            result
        }
        Err(format!("a == b expected for '{}'. Actual:\n{}\nExpected:\n{}\n", help(), format(actual), format(expected)))
    }
}

/// Tests if `v1 == v2`, looking at the exact bit representation
pub fn soft_assert_f32_bits(v1: f32, v2: f32, help: &str) -> Result<(), String> {
    let u1: u32 = unsafe { transmute(v1) };
    let u2: u32 = unsafe { transmute(v2) };
    if u1 == u2 {
        Ok(())
    } else {
        Err(format!("a == b expected, but a={:?} b={:?} (0x{:x} vs 0x{:x}). {}", v1, v2, u1, u2, help))
    }
}

/// Tests if `v1 == v2`, looking at the exact bit representation
pub fn soft_assert_f64_bits(v1: f64, v2: f64, help: &str) -> Result<(), String> {
    let u1: u64 = unsafe { transmute(v1) };
    let u2: u64 = unsafe { transmute(v2) };
    if u1 == u2 {
        Ok(())
    } else {
        Err(format!("a == b expected, but a={:?} b={:?} (0x{:x} vs 0x{:x}). {}", v1, v2, u1, u2, help))
    }
}

/// Tests if `v1 != v2`.
pub fn soft_assert_neq<T: Display + LowerHex + PartialEq + Eq>(v1: T, v2: T, help: &str) -> Result<(), String> {
    if v1 != v2 {
        Ok(())
    } else {
        Err(format!("a != b expected, but a={} b={} (hex: a=0x{:x} b=0x{:x}). {}", v1, v2, v1, v2, help))
    }
}

/// Tests if `v1 >= v2`.
pub fn soft_assert_greater_or_equal(v1: u32, v2: u32, help: &str) -> Result<(), String> {
    if v1 >= v2 {
        Ok(())
    } else {
        Err(format!("a >= b expected, but a={} b={} (hex: a=0x{:x} b=0x{:x}). {}", v1, v2, v1, v2, help))
    }
}

/// Tests if `v1 < v2`.
pub fn soft_assert_less(v1: u32, v2: u32, help: &str) -> Result<(), String> {
    if v1 < v2 {
        Ok(())
    } else {
        Err(format!("a < b expected, but a={} b={} (hex: a=0x{:x} b=0x{:x}). {}", v1, v2, v1, v2, help))
    }
}

/// Tests if `v1 < v2`.
pub fn soft_assert_range_contained_within_expected<T: PartialOrd + Ord + Debug>(expected_range: RangeInclusive<T>, seen_range: RangeInclusive<T>, help: &str) -> Result<(), String> {
    if expected_range.start() <= seen_range.start() && expected_range.end() >= seen_range.end() {
        Ok(())
    } else {
        Err(format!("Seen range {:?}, which was expected to be within range {:?}. {}", seen_range, expected_range, help))
    }
}

/// Tests if value is within range.
pub fn soft_assert_range<T: PartialOrd + LowerHex>(value: T, min: T, max: T, help: &str) -> Result<(), String> {
    if value >= min && value <= max {
        Ok(())
    } else {
        Err(format!("value expected to be 0x{:x}..=0x{:x}, but was 0x{:x}. {}", min, max, value, help))
    }
}
