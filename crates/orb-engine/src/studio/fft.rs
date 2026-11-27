//! In-place radix-2 FFT. Plans and work buffers are prepared outside the audio callback.

#[derive(Clone, Copy, Default, Debug)]
pub struct Complex {
    pub re: f64,
    pub im: f64,
}

impl Complex {
    pub fn mul(self, b: Self) -> Self {
        Self {
            re: self.re * b.re - self.im * b.im,
            im: self.re * b.im + self.im * b.re,
        }
    }
}

pub struct Fft {
    reverse: Vec<usize>,
    roots: Vec<Complex>,
}

impl Fft {
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 2);
        Self {
            reverse: (0..n)
                .map(|i| i.reverse_bits() >> (usize::BITS - n.ilog2()))
                .collect(),
            roots: (0..n / 2)
                .map(|i| {
                    let a = -std::f64::consts::TAU * i as f64 / n as f64;
                    Complex {
                        re: a.cos(),
                        im: a.sin(),
                    }
                })
                .collect(),
        }
    }

    pub fn run(&self, x: &mut [Complex], inverse: bool) {
        let n = self.reverse.len();
        assert_eq!(x.len(), n);
        for (i, &j) in self.reverse.iter().enumerate() {
            if i < j {
                x.swap(i, j);
            }
        }
        let mut width = 2;
        while width <= n {
            // Width divides n exactly at each radix-2 stage. Walk precomputed
            // roots by that stride rather than dividing inside every butterfly.
            let stride = n / width;
            for group in x.chunks_exact_mut(width) {
                let (left, right) = group.split_at_mut(width / 2);
                for ((low, high), &root) in left.iter_mut().zip(right).zip(self.roots.iter().step_by(stride)) {
                    let mut w = root;
                    if inverse { w.im = -w.im; }
                    let b = high.mul(w);
                    let a = *low;
                    *low = Complex { re: a.re + b.re, im: a.im + b.im };
                    *high = Complex { re: a.re - b.re, im: a.im - b.im };
                }
            }
            width *= 2;
        }
        if inverse {
            for a in x {
                a.re /= n as f64;
                a.im /= n as f64;
            }
        }
    }
}
