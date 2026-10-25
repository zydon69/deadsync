//! Compatibility PRNG for seeded System.Random used by GrooveAuthor. The
//! generator seed must reproduce reference choices, independent of platform.
pub struct SeededRandom {
    seed: [i32; 56],
    next: usize,
    next_p: usize,
}

impl SeededRandom {
    pub fn new(seed: i32) -> Self {
        const BIG: i32 = i32::MAX;
        let subtraction = if seed == i32::MIN { BIG } else { seed.abs() };
        let mut mj = 161_803_398 - subtraction;
        let mut values = [0; 56];
        values[55] = mj;
        let mut mk = 1;
        for i in 1..55 {
            let ii = (21 * i) % 55;
            values[ii] = mk;
            mk = mj.wrapping_sub(mk);
            if mk < 0 {
                mk = mk.wrapping_add(BIG);
            }
            mj = values[ii];
        }
        for _ in 0..4 {
            for i in 1..56 {
                let value = values[i].wrapping_sub(values[1 + (i + 30) % 55]);
                values[i] = if value < 0 {
                    value.wrapping_add(BIG)
                } else {
                    value
                };
            }
        }
        Self {
            seed: values,
            next: 0,
            next_p: 21,
        }
    }

    pub fn next_i32(&mut self) -> i32 {
        self.next = if self.next + 1 >= 56 {
            1
        } else {
            self.next + 1
        };
        self.next_p = if self.next_p + 1 >= 56 {
            1
        } else {
            self.next_p + 1
        };
        let mut value = self.seed[self.next].wrapping_sub(self.seed[self.next_p]);
        if value == i32::MAX {
            value -= 1;
        }
        if value < 0 {
            value = value.wrapping_add(i32::MAX);
        }
        self.seed[self.next] = value;
        value
    }

    pub fn next_f64(&mut self) -> f64 {
        f64::from(self.next_i32()) * (1.0 / f64::from(i32::MAX))
    }
}
