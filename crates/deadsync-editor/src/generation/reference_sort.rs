//! .NET ArraySortHelper<T> comparer-based introsort, runtime v10.0.13 (MIT).
//! Equivalent fallback links must keep the reference's tie permutation: using
//! Rust's stable sort would change fallback indices and seeded conversion.
//! Notice: docs/editor/licenses/dotnet-runtime-MIT.txt.
use std::cmp::Ordering;

pub fn sort<T: Copy>(values: &mut [T], compare: impl Fn(&T, &T) -> Ordering) {
    if values.len() > 1 {
        let depth = 2 * (values.len().ilog2() + 1);
        intro(values, depth, &compare);
    }
}
fn swap_greater<T>(values: &mut [T], a: usize, b: usize, compare: &impl Fn(&T, &T) -> Ordering) {
    if compare(&values[a], &values[b]) == Ordering::Greater {
        values.swap(a, b);
    }
}
fn intro<T: Copy>(values: &mut [T], mut depth: u32, compare: &impl Fn(&T, &T) -> Ordering) {
    let mut size = values.len();
    while size > 1 {
        if size <= 16 {
            if size == 2 {
                swap_greater(values, 0, 1, compare);
                return;
            }
            if size == 3 {
                swap_greater(values, 0, 1, compare);
                swap_greater(values, 0, 2, compare);
                swap_greater(values, 1, 2, compare);
                return;
            }
            for i in 1..size {
                let value = values[i];
                let mut j = i;
                while j > 0 && compare(&value, &values[j - 1]) == Ordering::Less {
                    values[j] = values[j - 1];
                    j -= 1;
                }
                values[j] = value;
            }
            return;
        }
        if depth == 0 {
            heap(&mut values[..size], compare);
            return;
        }
        depth -= 1;
        let pivot = partition(&mut values[..size], compare);
        intro(&mut values[pivot + 1..size], depth, compare);
        size = pivot;
    }
}
fn partition<T: Copy>(values: &mut [T], compare: &impl Fn(&T, &T) -> Ordering) -> usize {
    let high = values.len() - 1;
    let middle = high / 2;
    swap_greater(values, 0, middle, compare);
    swap_greater(values, 0, high, compare);
    swap_greater(values, middle, high, compare);
    let pivot = values[middle];
    values.swap(middle, high - 1);
    let (mut left, mut right) = (0, high - 1);
    while left < right {
        loop {
            left += 1;
            if compare(&values[left], &pivot) != Ordering::Less {
                break;
            }
        }
        loop {
            right -= 1;
            if compare(&pivot, &values[right]) != Ordering::Less {
                break;
            }
        }
        if left >= right {
            break;
        }
        values.swap(left, right);
    }
    if left != high - 1 {
        values.swap(left, high - 1);
    }
    left
}
fn heap<T: Copy>(values: &mut [T], compare: &impl Fn(&T, &T) -> Ordering) {
    let length = values.len();
    for i in (1..=length / 2).rev() {
        down(values, i, length, compare);
    }
    for i in (2..=length).rev() {
        values.swap(0, i - 1);
        down(values, 1, i - 1, compare);
    }
}
fn down<T: Copy>(
    values: &mut [T],
    mut i: usize,
    length: usize,
    compare: &impl Fn(&T, &T) -> Ordering,
) {
    let value = values[i - 1];
    while i <= length / 2 {
        let mut child = 2 * i;
        if child < length && compare(&values[child - 1], &values[child]) == Ordering::Less {
            child += 1;
        }
        if compare(&value, &values[child - 1]) != Ordering::Less {
            break;
        }
        values[i - 1] = values[child - 1];
        i = child;
    }
    values[i - 1] = value;
}
