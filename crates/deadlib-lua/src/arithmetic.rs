//! Lua 5.1 arithmetic shared by constant folding and runtime evaluation.
pub(crate) fn modulo(x: f64, y: f64) -> f64 {
    let quotient = (x / y).floor();
    // The reference C build contracts this multiply/subtract on AArch64.
    // Keep the same rounding in folded literals and live register arithmetic.
    #[cfg(target_arch = "aarch64")]
    {
        (-quotient).mul_add(y, x)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        x - quotient * y
    }
}
