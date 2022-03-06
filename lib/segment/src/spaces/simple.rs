#[cfg(target_arch = "x86")]
use std::arch::x86::*;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

#[cfg(target_arch = "arm")]
use std::arch::arm::*;

use crate::types::{Distance, ScoreType, VectorElementType};

use super::metric::Metric;

pub struct DotProductMetric {}

pub struct CosineMetric {}

pub struct EuclidMetric {}

impl Metric for EuclidMetric {
    fn distance(&self) -> Distance {
        Distance::Euclid
    }

    fn similarity(&self, v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { euclid_similarity_avx2(v1, v2) };
            }
            if is_x86_feature_detected!("sse") {
                return unsafe { euclid_similarity_sse(v1, v2) };
            }
        }
        euclid_similarity(v1, v2)
    }

    fn preprocess(&self, _vector: &[VectorElementType]) -> Option<Vec<VectorElementType>> {
        None
    }
}

impl Metric for DotProductMetric {
    fn distance(&self) -> Distance {
        Distance::Dot
    }

    fn similarity(&self, v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { dot_similarity_avx2(v1, v2) };
            }
            if is_x86_feature_detected!("sse") {
                return unsafe { dot_similarity_sse(v1, v2) };
            }
        }
        dot_similarity(v1, v2)
    }

    fn preprocess(&self, _vector: &[VectorElementType]) -> Option<Vec<VectorElementType>> {
        None
    }
}

impl Metric for CosineMetric {
    fn distance(&self) -> Distance {
        Distance::Cosine
    }

    fn similarity(&self, v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                return unsafe { dot_similarity_avx2(v1, v2) };
            }
            if is_x86_feature_detected!("sse") {
                return unsafe { dot_similarity_sse(v1, v2) };
            }
        }
        dot_similarity(v1, v2)
    }

    fn preprocess(&self, vector: &[VectorElementType]) -> Option<Vec<VectorElementType>> {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_feature_detected!("avx2") {
                return Some(unsafe { cosine_preprocess_avx2(vector) });
            }
            if is_x86_feature_detected!("sse") {
                return Some(unsafe { cosine_preprocess_sse(vector) });
            }
        }
        Some(cosine_preprocess(vector))
    }
}

fn euclid_similarity(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let s: ScoreType = v1
        .iter()
        .copied()
        .zip(v2.iter().copied())
        .map(|(a, b)| (a - b).powi(2))
        .sum();
    -s.sqrt()
}

fn cosine_preprocess(vector: &[VectorElementType]) -> Vec<VectorElementType> {
    let mut length: f32 = vector.iter().map(|x| x * x).sum();
    length = length.sqrt();
    vector.iter().map(|x| x / length).collect()
}

fn dot_similarity(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    v1.iter().zip(v2).map(|(a, b)| a * b).sum()
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx512f"))]
unsafe fn euclid_similarity_avx512f(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let n2 = v1.len();
    let m = n - (n % 16);
    let mut sum512: __m512 = _mm512_setzero_ps();
    for i in (0..m).step_by(16) {
        let sub512: __m512 = _mm512_sub_ps(_mm512_loadu_ps(&v1[i]), _mm512_loadu_ps(&v2[i]));
        sum512 = _mm512_fmadd_ps(sub512, sub512, sum512);
    }
    let mut res = _mm512_mask_reduce_add_ps(u16::MAX, sum512);
    for i in m..n {
        res += (v1[i] - v2[i]).powi(2);
    }
    -res.sqrt()
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx512f"))]
unsafe fn cosine_preprocess_avx512f(vector: &[VectorElementType]) -> Vec<VectorElementType> {
    let n = vector.len();
    let m = n - (n % 16);
    let mut sum512: __m512 = _mm512_setzero_ps();
    for i in (0..m).step_by(16) {
        sum512 = _mm512_fmadd_ps(
            _mm512_loadu_ps(&vector[i]),
            _mm512_loadu_ps(&vector[i]),
            sum512,
        );
    }
    let mut length = _mm512_mask_reduce_add_ps(u16::MAX, sum512);
    for v in vector.iter().take(n).skip(m) {
        length += v.powi(2);
    }
    length = length.sqrt();
    vector.iter().map(|x| x / length).collect()
}

#[cfg(all(
    target_arch = "x86_64",
    target_feature = "avx512f"))]
unsafe fn dot_similarity_avx512f(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let n = v1.len();
    let m = n - (n % 16);
    let mut sum512: __m512 = _mm512_setzero_ps();
    for i in (0..m).step_by(16) {
        sum512 = _mm512_fmadd_ps(_mm512_loadu_ps(&v1[i]), _mm512_loadu_ps(&v2[i]), sum512);
    }
    let mut res = _mm512_mask_reduce_add_ps(u16::MAX, sum512);
    for i in m..n {
        res += v1[i] * v2[i];
    }
    res
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn hsum256_ps_avx2(x: __m256) -> f32 {
    /* ( x3+x7, x2+x6, x1+x5, x0+x4 ) */
    let x128: __m128 = _mm_add_ps(_mm256_extractf128_ps(x, 1), _mm256_castps256_ps128(x));
    /* ( -, -, x1+x3+x5+x7, x0+x2+x4+x6 ) */
    let x64: __m128 = _mm_add_ps(x128, _mm_movehl_ps(x128, x128));
    /* ( -, -, -, x0+x1+x2+x3+x4+x5+x6+x7 ) */
    let x32: __m128 = _mm_add_ss(x64, _mm_shuffle_ps(x64, x64, 0x55));
    /* Conversion to float is a no-op on x86-64 */
    _mm_cvtss_f32(x32)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn euclid_similarity_avx2(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let n = v1.len();
    let m = n - (n % 8);
    let mut sum256: __m256 = _mm256_setzero_ps();
    for i in (0..m).step_by(8) {
        let sub256: __m256 = _mm256_sub_ps(_mm256_loadu_ps(&v1[i]), _mm256_loadu_ps(&v2[i]));
        sum256 = _mm256_fmadd_ps(sub256, sub256, sum256);
    }
    let mut res = hsum256_ps_avx2(sum256);
    for i in m..n {
        res += (v1[i] - v2[i]).powi(2);
    }
    -res.sqrt()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn cosine_preprocess_avx2(vector: &[VectorElementType]) -> Vec<VectorElementType> {
    let n = vector.len();
    let m = n - (n % 8);
    let mut sum256: __m256 = _mm256_setzero_ps();
    for i in (0..m).step_by(8) {
        sum256 = _mm256_fmadd_ps(
            _mm256_loadu_ps(&vector[i]),
            _mm256_loadu_ps(&vector[i]),
            sum256,
        );
    }
    let mut length = hsum256_ps_avx2(sum256);
    for v in vector.iter().take(n).skip(m) {
        length += v.powi(2);
    }
    length = length.sqrt();
    vector.iter().map(|x| x / length).collect()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn dot_similarity_avx2(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let n = v1.len();
    let m = n - (n % 8);
    let mut sum256: __m256 = _mm256_setzero_ps();
    for i in (0..m).step_by(8) {
        sum256 = _mm256_fmadd_ps(_mm256_loadu_ps(&v1[i]), _mm256_loadu_ps(&v2[i]), sum256);
    }
    let mut res = hsum256_ps_avx2(sum256);
    for i in m..n {
        res += v1[i] * v2[i];
    }
    res
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse")]
unsafe fn hsum128_ps_sse(x: __m128) -> f32 {
    let x64: __m128 = _mm_add_ps(x, _mm_movehl_ps(x, x));
    let x32: __m128 = _mm_add_ss(x64, _mm_shuffle_ps(x64, x64, 0x55));
    _mm_cvtss_f32(x32)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse")]
unsafe fn euclid_similarity_sse(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let n = v1.len();
    let m = n - (n % 4);
    let mut sum128: __m128 = _mm_setzero_ps();
    for i in (0..m).step_by(4) {
        let sub128: __m128 = _mm_sub_ps(_mm_loadu_ps(&v1[i]), _mm_loadu_ps(&v2[i]));
        let a = _mm_mul_ps(sub128, sub128);
        sum128 = _mm_add_ps(a, sum128);
    }
    let mut res = hsum128_ps_sse(sum128);
    for i in m..n {
        res += (v1[i] - v2[i]).powi(2);
    }
    -res.sqrt()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse")]
unsafe fn cosine_preprocess_sse(vector: &[VectorElementType]) -> Vec<VectorElementType> {
    let n = vector.len();
    let m = n - (n % 4);
    let mut sum128: __m128 = _mm_setzero_ps();
    for i in (0..m).step_by(4) {
        let a = _mm_loadu_ps(&vector[i]);
        let b = _mm_mul_ps(a, a);
        sum128 = _mm_add_ps(b, sum128);
    }
    let mut length = hsum128_ps_sse(sum128);
    for v in vector.iter().take(n).skip(m) {
        length += v.powi(2);
    }
    length = length.sqrt();
    vector.iter().map(|x| x / length).collect()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse")]
unsafe fn dot_similarity_sse(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let n = v1.len();
    let m = n - (n % 4);
    let mut sum128: __m128 = _mm_setzero_ps();
    for i in (0..m).step_by(4) {
        let a = _mm_loadu_ps(&v1[i]);
        let b = _mm_loadu_ps(&v2[i]);
        let c = _mm_mul_ps(a, b);
        sum128 = _mm_add_ps(c, sum128);
    }
    let mut res = hsum128_ps_sse(sum128);
    for i in m..n {
        res += v1[i] * v2[i];
    }
    res
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon"))]
unsafe fn euclid_similarity_neon(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let n = v1.len();
    let m = n - (n % 4);
    let mut res : f64 = 0.0;
    for i in (0..m).step_by(4) {
        let a = vld1q_f32(&v1[i]);
        let b = vld1q_f32(&v2[i]);
        let c = vsubq_f32(a, b);
        let d = vmulq_f32(c, c);
        res += vaddvq_f32(d) as f64;
    }
    for i in m..n {
        res += (v1[i] - v2[i]).powi(2) as f64;
    }
    -res.sqrt() as ScoreType
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon"))]
unsafe fn cosine_preprocess_neon(vector: &[VectorElementType]) -> Vec<VectorElementType> {
    let n = vector.len();
    let m = n - (n % 4);
    let mut length : f64 = 0.0;
    for i in (0..m).step_by(4) {
        let a = vld1q_f32(&vector[i]);
        let b = vmulq_f32(a, a);
        length += vaddvq_f32(b) as f64;
    }
    for v in vector.iter().take(n).skip(m) {
        length += v.powi(2) as f64;
    }
    let length = length.sqrt() as f32;
    vector.iter().map(|x| x / length).collect()
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon"))]
unsafe fn dot_similarity_neon(v1: &[VectorElementType], v2: &[VectorElementType]) -> ScoreType {
    let n = v1.len();
    let m = n - (n % 4);
    let mut res : f64 = 0.0;
    for i in (0..m).step_by(4) {
        let a = vld1q_f32(&v1[i]);
        let b = vld1q_f32(&v2[i]);
        let c = vmulq_f32(a, b);
        res += vaddvq_f32(c) as f64;
    }
    for i in m..n {
        res += (v1[i] * v2[i]) as f64;
    }
    res as ScoreType
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_preprocessing() {
        let metric = CosineMetric {};
        let res = metric.preprocess(&[0.0, 0.0, 0.0, 0.0]);
        eprintln!("res = {:#?}", res);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn test_simd() {
        #[cfg(all(
            target_arch = "x86_64",
            target_feature = "avx512f"))]
        {
            if is_x86_feature_detected!("avx512f") {
                println!("avx512f test passed");

                let v1: Vec<f32> = vec![
                    10., 11., 12., 13., 14., 15., 16., 17., 18., 19., 20., 21., 22., 23., 24., 25.,
                    26., 27., 28., 29., 30., 31.,
                ];
                let v2: Vec<f32> = vec![
                    40., 41., 42., 43., 44., 45., 46., 47., 48., 49., 50., 51., 52., 53., 54., 55.,
                    56., 57., 58., 59., 60., 61.,
                ];
    
                let euclid_simd = unsafe { euclid_similarity_avx512f(&v1, &v2) };
                let euclid = euclid_similarity(&v1, &v2);
                assert_eq!(euclid_simd, euclid);
    
                let dot_simd = unsafe { dot_similarity_avx512f(&v1, &v2) };
                let dot = dot_similarity(&v1, &v2);
                assert_eq!(dot_simd, dot);
    
                let cosine_simd = unsafe { cosine_preprocess_avx512f(&v1) };
                let cosine = cosine_preprocess(&v1);
                assert_eq!(cosine_simd, cosine);
            } else {
                println!("avx512f test skiped");
            }
        }

        if is_x86_feature_detected!("sse") {
            let v1: Vec<f32> = vec![
                10., 11., 12., 13., 14., 15., 16., 17., 18., 19., 20., 21., 22., 23., 24., 25.,
                26., 27., 28., 29., 30., 31.,
            ];
            let v2: Vec<f32> = vec![
                40., 41., 42., 43., 44., 45., 46., 47., 48., 49., 50., 51., 52., 53., 54., 55.,
                56., 57., 58., 59., 60., 61.,
            ];

            let euclid_simd = unsafe { euclid_similarity_sse(&v1, &v2) };
            let euclid = euclid_similarity(&v1, &v2);
            assert_eq!(euclid_simd, euclid);

            let dot_simd = unsafe { dot_similarity_sse(&v1, &v2) };
            let dot = dot_similarity(&v1, &v2);
            assert_eq!(dot_simd, dot);

            let cosine_simd = unsafe { cosine_preprocess_sse(&v1) };
            let cosine = cosine_preprocess(&v1);
            assert_eq!(cosine_simd, cosine);
        } else {
            println!("SSE test skiped");
        }

        if is_x86_feature_detected!("avx2") {
            let v1: Vec<f32> = vec![
                10., 11., 12., 13., 14., 15., 16., 17., 18., 19., 20., 21., 22., 23., 24., 25.,
                26., 27., 28., 29., 30., 31.,
            ];
            let v2: Vec<f32> = vec![
                40., 41., 42., 43., 44., 45., 46., 47., 48., 49., 50., 51., 52., 53., 54., 55.,
                56., 57., 58., 59., 60., 61.,
            ];

            let euclid_simd = unsafe { euclid_similarity_avx2(&v1, &v2) };
            let euclid = euclid_similarity(&v1, &v2);
            assert_eq!(euclid_simd, euclid);

            let dot_simd = unsafe { dot_similarity_avx2(&v1, &v2) };
            let dot = dot_similarity(&v1, &v2);
            assert_eq!(dot_simd, dot);

            let cosine_simd = unsafe { cosine_preprocess_avx2(&v1) };
            let cosine = cosine_preprocess(&v1);
            assert_eq!(cosine_simd, cosine);
        } else {
            println!("AVX2 test skiped");
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn test_neon() {
        if std::arch::is_aarch64_feature_detected!("neon") {
            let v1: Vec<f32> = vec![
                10., 11., 12., 13., 14., 15., 16., 17., 18., 19., 20., 21., 22., 23., 24., 25.,
                26., 27., 28., 29., 30., 31.,
            ];
            let v2: Vec<f32> = vec![
                40., 41., 42., 43., 44., 45., 46., 47., 48., 49., 50., 51., 52., 53., 54., 55.,
                56., 57., 58., 59., 60., 61.,
            ];

            let euclid_simd = unsafe { euclid_similarity_neon(&v1, &v2) };
            let euclid = euclid_similarity(&v1, &v2);
            assert_eq!(euclid_simd, euclid);

            let dot_simd = unsafe { dot_similarity_neon(&v1, &v2) };
            let dot = dot_similarity(&v1, &v2);
            assert_eq!(dot_simd, dot);

            let cosine_simd = unsafe { cosine_preprocess_neon(&v1) };
            let cosine = cosine_preprocess(&v1);
            assert_eq!(cosine_simd, cosine);
        } else {
            println!("neon test skiped");
        }
    }
}
