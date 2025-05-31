use std::iter::once;

use chrono::{DateTime, TimeDelta, Utc};
use log::debug;
use shared::api::{dose::CreateDose, medication::DoseLimit};

#[allow(unused_imports)] // Maybe we can remove this dependency
use itertools::Itertools;

#[allow(dead_code)] // TODO
fn next_allowed(doses: &[CreateDose], limits: &[DoseLimit]) -> Option<Vec<CreateDose>> {
    let min_quantity_limit = limits
        .iter()
        .map(|l| l.amount)
        .min_by(|a, b| a.total_cmp(b))?;

    let last_non_zero = doses
        .iter()
        .filter(|dose| dose.quantity > 0.0)
        .next_back()?;

    let times_to_check = doses
        .iter()
        .flat_map(|dose| {
            limits
                .iter()
                .map(|lim| {
                    dose.taken_at
                    .checked_add_signed(TimeDelta::hours(lim.hours.into())).unwrap(/*TODO */)
                })
                .collect::<Vec<_>>()
        })
        .filter(|t| *t > last_non_zero.taken_at);

    let times_to_check = once(last_non_zero.taken_at)
        .chain(times_to_check)
        .collect::<Vec<_>>();

    let when_min_quantity_limit = times_to_check
        .iter()
        .filter(|t| {
            limits
                .iter()
                .all(|lim| check_allowed(lim, doses, t) >= min_quantity_limit)
        })
        .inspect(|t| log::debug!("This time allows {min_quantity_limit:?}: {t}"))
        .min();

    // TODO dedup

    let when_nonzero = times_to_check
        .iter()
        .map(|t| {
            (
                t,
                limits
                    .iter()
                    .map(|lim| check_allowed(lim, doses, t))
                    .min_by(|a, b| a.total_cmp(b))
                    .unwrap(),
            )
        })
        .inspect(|t| log::debug!("{t:?}"))
        .filter(|(_t, amount)| *amount > 0.0)
        .min_by_key(|(t, _amount)| *t);

    // TODO: Lots of unwrap and duplication in this function

    let result = [
        CreateDose {
            quantity: when_nonzero.unwrap().1,
            taken_at: *when_nonzero.unwrap().0,
            noted_by_user: None,
        },
        CreateDose {
            quantity: min_quantity_limit,
            taken_at: *when_min_quantity_limit.unwrap(),
            noted_by_user: None,
        },
    ];

    if result[0] == result[1] {
        // TODO: Avoid this clone?
        Some(vec![result[0].clone()])
    } else {
        Some(result.into())
    }
}

// TODO: Move the various functions here into an Ext trait on DoseLimit

// TODO: Create tests for this
fn check_allowed(limit: &DoseLimit, history: &[CreateDose], time: &DateTime<Utc>) -> f64 {
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

#[allow(dead_code)] // TODO
fn next_allowed_single(doses: &[CreateDose], limit: &DoseLimit) -> Option<Vec<CreateDose>> {
    let last_non_zero = doses
        .iter()
        .filter(|dose| dose.quantity > 0.0)
        .next_back()?;

    let hours = TimeDelta::hours(limit.hours.into());
    let epoch_start = last_non_zero
        .taken_at
        .checked_sub_signed(hours)
        .log_error_if_none("Time overflow computing epoch")?;

    let next_full_dose = last_non_zero
        .taken_at
        .checked_add_signed(hours)
        .log_error_if_none("Time overflow computing next full dose")?;

    let total: f64 = doses
        .iter()
        .filter(|dose| dose.taken_at > epoch_start)
        .map(|dose| dose.quantity)
        .sum();

    debug!("last_non_zero: {last_non_zero:?}");
    debug!("next_full_dose: {next_full_dose:?}");
    debug!("total: {total:?}");

    let mut result = vec![];

    if total < limit.amount {
        result.push(CreateDose {
            quantity: limit.amount - total,
            taken_at: last_non_zero.taken_at,
            noted_by_user: None,
        });
    } else if let Some(partial_dose) = earliest_possible_partial_dose(doses, limit.amount, hours) {
        result.push(partial_dose);
    }

    result.push(CreateDose {
        quantity: limit.amount,
        taken_at: next_full_dose,
        noted_by_user: None,
    });

    Some(result)
}

fn earliest_possible_partial_dose(
    doses: &[CreateDose],
    limit_amount: f64,
    hours: TimeDelta,
) -> Option<CreateDose> {
    let mut current_total: f64 = doses.iter().map(|dose| dose.quantity).sum();

    for dose in doses.iter() {
        current_total -= dose.quantity;
        debug!("After decreasing {dose:?}, current_total is {current_total}");
        if current_total < limit_amount {
            debug!("...which is under the limit!");
            if current_total > 0.0 {
                return Some(CreateDose {
                    quantity: limit_amount - current_total,
                    taken_at: dose
                        .taken_at
                        .checked_add_signed(hours)
                        .log_error_if_none("Time overflow computing earliest partial dose")?,
                    noted_by_user: None,
                });
            } else {
                debug!("...but we're already adding the 0-total option below.");
                return None;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta, TimeZone, Utc};
    use pretty_assertions::assert_eq;
    use rstest::rstest;
    use shared::api::{dose::CreateDose, medication::DoseLimit};

    use pretty_env_logger::env_logger;

    fn init() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_module("trufotbot", log::LevelFilter::Debug)
            .format_timestamp(None)
            .try_init();
    }

    use super::next_allowed_single;

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
    fn test_check_allowed(
        #[case] limit: DoseLimit,
        #[case] history: DosesShortSyntax,
        #[case] candidate: DoseShortSyntax,
    ) {
        use crate::dose_limits::check_allowed;

        init();

        let history = from_short_syntax(history);
        let candidate = from_short_syntax_single(candidate);

        let got = check_allowed(&limit, &history, &candidate.taken_at);

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

        let got = next_allowed_single(&doses, &limit);

        assert_eq!(got, Some(want.clone()));

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

/*

See https://docs.google.com/spreadsheets/d/1O40kbDI6GNrwo-TA1fY2jXrUJ3mkuStUujU8AMS_CHI/edit?gid=0#gid=0
for worked examples

*/
