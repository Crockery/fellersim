//! Paired Monte Carlo intervals. These quantify sampling uncertainty, not model accuracy.
use crate::{ConfidenceInterval, SimulationError, SimulationErrorCode};
use serde::{Deserialize, Serialize};
use statrs::distribution::{ContinuousCDF, StudentsT};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ComparisonMetric {
    TotalDps,
    PrimaryTargetDps,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PairedComparison {
    pub baseline_mean: f64,
    pub variant_mean: f64,
    pub absolute_change: f64,
    pub percentage_change: Option<f64>,
    pub percentage_explanation: Option<String>,
    pub adjusted_interval: ConfidenceInterval,
    pub classification: String,
    pub family_confidence: f64,
    pub comparisons: usize,
    pub samples: usize,
    pub method: String,
}

pub fn compare_paired(
    baseline: &[f64],
    variant: &[f64],
    comparisons: usize,
) -> Result<PairedComparison, SimulationError> {
    if baseline.len() < 2
        || baseline.len() != variant.len()
        || comparisons == 0
        || baseline.iter().chain(variant).any(|v| !v.is_finite())
    {
        return Err(SimulationError::new(
            SimulationErrorCode::InvalidBuild,
            "Paired intervals require equal finite samples, at least two iterations, and a positive comparison count.",
        ));
    }
    let n = baseline.len() as f64;
    let base = baseline.iter().sum::<f64>() / n;
    let mean = variant.iter().sum::<f64>() / n;
    let differences = baseline
        .iter()
        .zip(variant)
        .map(|(b, v)| v - b)
        .collect::<Vec<_>>();
    let delta = differences.iter().sum::<f64>() / n;
    let variance = differences.iter().map(|d| (d - delta).powi(2)).sum::<f64>() / (n - 1.0);
    let critical = StudentsT::new(0.0, 1.0, n - 1.0)
        .expect("positive degrees of freedom")
        .inverse_cdf(1.0 - 0.05 / (2.0 * comparisons as f64));
    let margin = critical * (variance / n).sqrt();
    let interval = ConfidenceInterval {
        low: delta - margin,
        high: delta + margin,
    };
    let classification = if interval.low > 0.0 {
        "higher"
    } else if interval.high < 0.0 {
        "lower"
    } else {
        "inconclusive"
    };
    Ok(PairedComparison {
        baseline_mean: base,
        variant_mean: mean,
        absolute_change: delta,
        percentage_change: (base != 0.0).then(|| delta / base * 100.0),
        percentage_explanation: (base == 0.0)
            .then(|| "Baseline mean is zero; percentage change is undefined.".into()),
        adjusted_interval: interval,
        classification: classification.into(),
        family_confidence: 0.95,
        comparisons,
        samples: baseline.len(),
        method: "paired-student-t-bonferroni".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paired_covariance_and_zero_baseline() {
        let identical = compare_paired(&[1., 2., 3.], &[1., 2., 3.], 2).unwrap();
        assert_eq!(identical.adjusted_interval.low, 0.);
        assert_eq!(identical.classification, "inconclusive");
        let constant = compare_paired(&[1., 2., 3.], &[3., 4., 5.], 2).unwrap();
        assert_eq!(constant.adjusted_interval.low, 2.);
        assert_eq!(constant.classification, "higher");
        assert!(
            compare_paired(&[0., 0.], &[1., 2.], 1)
                .unwrap()
                .percentage_change
                .is_none()
        );
    }
    #[test]
    fn known_t_interval_and_family_adjustment() {
        let single = compare_paired(&[0.; 5], &[1., 2., 3., 4., 5.], 1).unwrap();
        assert!((single.adjusted_interval.high - 4.963243161).abs() < 1e-7);
        let family = compare_paired(&[0.; 5], &[1., 2., 3., 4., 5.], 3).unwrap();
        assert!(family.adjusted_interval.high > single.adjusted_interval.high);
        assert!(compare_paired(&[1.], &[2.], 1).is_err());
    }
}
