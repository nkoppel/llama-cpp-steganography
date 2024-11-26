use llama_cpp_2::token::{data::LlamaTokenData, data_array::LlamaTokenDataArray, LlamaToken};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

#[derive(Clone, Debug, Default)]
pub struct LogitVector(pub Vec<f32>);

impl AddAssign<&Self> for LogitVector {
    fn add_assign(&mut self, rhs: &Self) {
        for (s, r) in self.0.iter_mut().zip(&rhs.0) {
            *s += *r;
        }
    }
}

impl SubAssign<&Self> for LogitVector {
    fn sub_assign(&mut self, rhs: &Self) {
        for (s, r) in self.0.iter_mut().zip(&rhs.0) {
            *s -= *r;
        }
    }
}

impl MulAssign<f32> for LogitVector {
    fn mul_assign(&mut self, rhs: f32) {
        for s in &mut self.0 {
            *s *= rhs;
        }
    }
}

#[allow(clippy::suspicious_op_assign_impl)]
impl DivAssign<f32> for LogitVector {
    fn div_assign(&mut self, rhs: f32) {
        *self *= rhs.recip();
    }
}

impl Add<&Self> for LogitVector {
    type Output = Self;

    fn add(mut self, rhs: &Self) -> Self {
        self += rhs;
        self
    }
}

impl Sub<&Self> for LogitVector {
    type Output = Self;

    fn sub(mut self, rhs: &Self) -> Self {
        self -= rhs;
        self
    }
}

impl Mul<f32> for LogitVector {
    type Output = Self;

    fn mul(mut self, rhs: f32) -> Self {
        self *= rhs;
        self
    }
}

impl Div<f32> for LogitVector {
    type Output = Self;

    fn div(mut self, rhs: f32) -> Self {
        self /= rhs;
        self
    }
}

impl LogitVector {
    pub fn from_token_data(token_data: &LlamaTokenDataArray) -> Self {
        assert!(token_data.data.is_sorted_by_key(|data| data.id()));
        Self(token_data.data.iter().map(|data| data.logit()).collect())
    }

    pub fn from_logits(logits: &[f32]) -> Self {
        Self(logits.to_vec())
    }

    pub fn to_token_data(&self) -> LlamaTokenDataArray {
        LlamaTokenDataArray::from_iter(
            self.0
                .iter()
                .enumerate()
                .map(|(i, logit)| LlamaTokenData::new(LlamaToken(i as i32), *logit, 0.)),
            false,
        )
    }

    pub fn dot(&self, other: &Self) -> f32 {
        self.0.iter().zip(&other.0).map(|(r, l)| r * l).sum()
    }

    pub fn norm(&self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn average<'a>(mut iter: impl Iterator<Item = &'a Self>) -> Self {
        let Some(mut out) = iter.next().cloned() else {
            return LogitVector(Vec::new());
        };

        let mut vecs = 1;

        for vec in iter {
            out += vec;
            vecs += 1;
        }
        out /= vecs as f32;

        out
    }

    pub fn project(&self, other: &Self) -> Self {
        other.clone() * (self.dot(other) / other.dot(other))
    }

    pub fn negate_along(&self, other: &Self) -> Self {
        self.project(other) * -2. + self
    }

    pub fn orthogonalize(&mut self, plane: &Self) {
        *self -= &self.project(plane);
    }

    pub fn orthogonalize_onto(&mut self, plane: &Self) {
        self.orthogonalize(plane);
        *self += plane;
    }
}
