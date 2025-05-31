use std::iter::once;

use chrono::{DateTime, TimeDelta, Utc};
use shared::api::{dose::CreateDose, medication::DoseLimit};

#[allow(unused_imports)] // Maybe we can remove this dependency
use itertools::Itertools;

fn times_to_check(doses: &[CreateDose], limits: &[DoseLimit]) -> Option<Vec<DateTime<Utc>>> {
    let last_non_zero_time = doses
        .iter()
        .filter(|dose| dose.quantity > 0.0)
        .next_back()?
        .taken_at;

    let candidate_times = doses
        .iter()
        .flat_map(|dose| {
            limits
                .iter()
                .map(|lim| {
                    dose.taken_at
                        .checked_add_signed(TimeDelta::hours(lim.hours.into()))
                        .log_error_if_none("Time overflow in next_allowed")
                })
                .collect::<Vec<_>>()
        })
        .filter(|t| match t {
            Some(t) => *t > last_non_zero_time,

            // Keep any None, which is a result of overflow, so that we collect
            // this as a None and reject the whole thing below
            None => true,
        });

    once(Some(last_non_zero_time))
        .chain(candidate_times)
        .collect::<Option<Vec<_>>>()
}

#[allow(dead_code)] // TODO
fn next_allowed(doses: &[CreateDose], limits: &[DoseLimit]) -> Option<Vec<CreateDose>> {
    fn compare_f64(a: &f64, b: &f64) -> std::cmp::Ordering {
        a.total_cmp(b)
    }

    // We'll count a "full dose" as whatever the tightest limit allows, as that's what you'd take "at once".
    let full_dose_quantity = limits.iter().map(|l| l.amount).min_by(compare_f64)?;

    let times_to_check = times_to_check(doses, limits)?;

    let full_dose = times_to_check
        .iter()
        .filter(|t| {
            limits
                .iter()
                .all(|lim| amount_allowed_at(lim, doses, t) >= full_dose_quantity)
        })
        .min();

    let any_dose = times_to_check
        .iter()
        .map(|t| {
            (
                t,
                limits
                    .iter()
                    .map(|lim| amount_allowed_at(lim, doses, t))
                    .min_by(compare_f64)
                    .unwrap(),
            )
        })
        .inspect(|t| log::debug!("{t:?}"))
        .filter(|(_t, amount)| *amount > 0.0)
        .min_by_key(|(t, _amount)| *t);

    let full_dose = CreateDose {
        quantity: full_dose_quantity,
        taken_at: *full_dose.unwrap(),
        noted_by_user: None,
    };

    let any_dose = CreateDose {
        quantity: any_dose.unwrap().1,
        taken_at: *any_dose.unwrap().0,
        noted_by_user: None,
    };

    if full_dose == any_dose {
        Some(vec![any_dose])
    } else {
        Some(vec![any_dose, full_dose])
    }
}

// TODO: Move the various functions here into an Ext trait on DoseLimit

fn amount_allowed_at(limit: &DoseLimit, history: &[CreateDose], time: &DateTime<Utc>) -> f64 {
    let duration = TimeDelta::hours(limit.hours.into());
    let epoch = time.checked_sub_signed(duration);
    let Some(epoch) = epoch else {
        log::error!(
            "Unexpected None in check_allowed; considering amount allowed at {time} to be 0.0"
        );
        return 0.0;
    };
    let total: f64 = history
        .iter()
        .rev()
        .filter(|d| d.taken_at > epoch)
        .map(|d| d.quantity)
        .sum();

    limit.amount - total
}

// The various DateTime checks have overflow checks. We can basically ignore
// them and return a "🤷", but we should log them if they happen.
trait LogIfNoneExt<T> {
    fn log_error_if_none(self, message: &str) -> Option<T>;
}

impl<T> LogIfNoneExt<T> for Option<T> {
    fn log_error_if_none(self, message: &str) -> Option<T> {
        if self.is_none() {
            log::error!("Unexpected None in {}: {message}", module_path!());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta, TimeZone, Utc};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use shared::api::{dose::CreateDose, medication::DoseLimit};

    use super::*;

    use pretty_env_logger::env_logger;

    fn init() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_module("trufotbot", log::LevelFilter::Debug)
            .format_timestamp(None)
            .try_init();
    }

    fn from_hm(hm: &str) -> TimeDelta {
        let (h, m) = hm.split_once(":").unwrap();
        TimeDelta::minutes(60 * h.parse::<i64>().unwrap() + m.parse::<i64>().unwrap())
    }

    fn base_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2023, 4, 5, 0, 0, 0).unwrap()
    }

    // TODO: Reimplement or remove based on DoseShortSyntax
    type DosesShortSyntax = &'static [(&'static str, f64)];

    fn from_short_syntax(doses: DosesShortSyntax) -> Vec<CreateDose> {
        doses
            .iter()
            .map(|(when, quantity)| CreateDose {
                quantity: *quantity,
                taken_at: base_time().checked_add_signed(from_hm(when)).unwrap(),
                noted_by_user: None,
            })
            .collect()
    }

    type DoseShortSyntax = (&'static str, f64);

    fn from_short_syntax_single((when, quantity): DoseShortSyntax) -> CreateDose {
        CreateDose {
            quantity,
            taken_at: base_time().checked_add_signed(from_hm(when)).unwrap(),
            noted_by_user: None,
        }
    }

    #[rstest]
    #[case::trivial(DoseLimit{ hours: 4, amount: 2.0 }, &[
        ("01:00", 2.0),
    ], ("05:00", 2.0))]
    #[case::too_early(DoseLimit{ hours: 4, amount: 2.0 }, &[
        ("01:00", 2.0),
    ], ("04:00", 0.0))]
    #[case::accumulated_exact(DoseLimit{ hours: 4, amount: 2.0 }, &[
        ("01:00", 1.0),
        ("02:00", 1.0),
    ], ("06:00", 2.0))]
    #[case::accumulated_too_early(DoseLimit{ hours: 4, amount: 2.0 }, &[
        ("01:00", 1.0),
        ("02:00", 1.0),
    ], ("05:00", 1.0))]
    fn test_amount_allowed_at(
        #[case] limit: DoseLimit,
        #[case] history: DosesShortSyntax,
        #[case] candidate: DoseShortSyntax,
    ) {
        init();

        let history = from_short_syntax(history);
        let candidate = from_short_syntax_single(candidate);

        let got = amount_allowed_at(&limit, &history, &candidate.taken_at);

        assert_eq!(got, candidate.quantity);
    }

    #[rstest]
    #[case::trivial(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("1:00", 3.5),
    ], &[("06:00", 3.5)])]
    #[case::one_partial_dose(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("1:00", 2.5),
    ], &[("01:00", 1.0), ("06:00", 3.5)])]
    #[case::two_partial_doses(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("1:00", 1.0),
            ("2:00", 1.0),
    ], &[("02:00", 1.5), ("07:00", 3.5)])]
    #[case::earlier_empty_dose(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("0:30", 0.0),
            ("1:00", 3.5),
    ], &[("06:00", 3.5)])]
    #[case::later_empty_dose(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("1:00", 3.5),
            ("1:30", 0.0),
    ], &[("06:00", 3.5)])]
    #[case::earlier_partial_and_then_full(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("0:30", 1.0),
            ("1:00", 3.5),
    ], &[("06:00", 3.5)])]
    #[case::full_and_then_partial(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("0:30", 3.5),
            ("1:00", 1.0),
    ], &[("05:30", 2.5), ("06:00", 3.5)])]
    #[case::full_and_then_two_partials(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("0:30", 3.5),
            ("1:00", 1.0),
            ("2:00", 1.0),
    ], &[("05:30", 1.5), ("07:00", 3.5)])]
    #[case::complex(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("1:00", 1.0),
            ("2:00", 1.0),
            ("3:00", 2.0),
            ("4:00", 1.0),
            ("5:00", 0.0),
    ], &[("07:00", 0.5), ("09:00", 3.5)])]
    fn test_single(
        #[case] limit: DoseLimit,
        #[case] doses: DosesShortSyntax,
        #[case] want: DosesShortSyntax,
    ) {
        use crate::dose_limits::next_allowed;

        init();

        let doses = from_short_syntax(doses);
        let want = from_short_syntax(want);

        let got = next_allowed(&doses, &[limit]);

        assert_eq!(got, Some(want));
    }

    #[rstest]
    #[case::trivial(&[
        DoseLimit{ hours: 4, amount: 2.0 },
    ], &[
            ("1:00", 2.0),
    ], &[("05:00", 2.0)])]
    #[case::trivial_two_rules(&[
        DoseLimit{ hours: 4, amount: 2.0 },
        DoseLimit{ hours: 24, amount: 8.0 },
    ], &[
            ("1:00", 2.0),
    ], &[("05:00", 2.0)])]
    #[case::two_rules_enforced(&[
        DoseLimit{ hours: 4, amount: 2.0 },
        DoseLimit{ hours: 20, amount: 8.0 },
    ], &[
            ("0:00", 2.0),
            ("4:00", 2.0),
            ("8:00", 2.0),
            ("12:00", 2.0),
    ], &[("20:00", 2.0)])]
    #[case::two_rules_partial(&[
        DoseLimit{ hours: 4, amount: 2.0 },
        DoseLimit{ hours: 20, amount: 8.0 },
    ], &[
            ("0:00", 2.0),
            ("4:00", 2.0),
            ("8:00", 2.0),
            ("12:00", 1.0),
    ], &[("12:00", 1.0), ("20:00", 2.0)])]
    fn test_multiple(
        #[case] limits: &[DoseLimit],
        #[case] doses: DosesShortSyntax,
        #[case] want: DosesShortSyntax,
    ) {
        use crate::dose_limits::next_allowed;

        init();

        let doses = from_short_syntax(doses);
        let want = from_short_syntax(want);

        let got = next_allowed(&doses, limits);

        assert_eq!(got, Some(want));
    }
}
