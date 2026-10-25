//! Equality used by the original editor's f64 setters (one ULP, signed zero equal).
pub(crate) fn reference_equal(a: f64, b: f64) -> bool {
    if a.to_bits() >> 63 != b.to_bits() >> 63 {
        a == b
    } else {
        a.to_bits().abs_diff(b.to_bits()) <= 1
    }
}
