use chrono::TimeDelta;
use log::debug;
use shared::api::{dose::CreateDose, medication::DoseLimit};

// #[allow(dead_code)] // TODO
// fn next_allowed(doses: &[CreateDose], limits: &[DoseLimit]) -> (DateTime<Utc>, DateTime<Utc>) {
//     todo!();
// }

#[allow(dead_code)] // TODO
fn next_allowed_single(doses: &[CreateDose], limit: &DoseLimit) -> Option<Vec<CreateDose>> {
    let last_non_zero = doses
        .iter()
        .filter(|dose| dose.quantity > 0.0)
        .next_back()?;

    let hours = TimeDelta::hours(limit.hours.into());
    let epoch_start = last_non_zero.taken_at.checked_sub_signed(hours) /* TODO: If this is None, it's out-of-range, log that */?;

    let next_full_dose = last_non_zero
        .taken_at
        .checked_add_signed(hours) /* TODO: If this is None, it's out-of-range, log that */ ?;

    let mut total: f64 = doses
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
    }

    for dose in doses.iter() {
        total -= dose.quantity;
        debug!("After decreasing {dose:?}, total is {total}");
        if total < limit.amount {
            debug!("...which is under the limit!");
            if total > 0.0 {
                result.push(
                CreateDose {
                    quantity: limit.amount - total,
                    taken_at: dose.taken_at.checked_add_signed(hours) /* TODO: If this is None, it's out-of-range, log that */?,
                    noted_by_user: None,
                });
            } else {
                debug!("...but we're already adding the 0-total option below.");
            }
            break;
        }
    }

    result.push(CreateDose {
        quantity: limit.amount,
        taken_at: next_full_dose,
        noted_by_user: None,
    });

    Some(result)
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

    #[rstest]
    #[case::trivial(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("1:00", 3.5),
    ], &[("06:00", 3.5)])]
    #[case::one_partial_dose(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("1:00", 2.5),
    ], &[("01:00", 1.0), ("06:00", 3.5)])]
    #[ignore] // TODO FIXME
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
    #[ignore] // TODO FIXME
    #[case::earlier_partial_and_then_full(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("0:30", 1.0),
            ("1:00", 3.5),
    ], &[("05:30", 2.5), ("06:00", 3.5)])]
    #[case::complex(DoseLimit{ hours: 5, amount: 3.5 }, &[
            ("1:00", 1.0),
            ("2:00", 1.0),
            ("3:00", 2.0),
            ("4:00", 1.0),
            ("5:00", 0.0),
    ], &[("07:00", 0.5), ("09:00", 3.5)])]
    // TODO: Add more cases
    fn test_a(
        #[case] limit: DoseLimit,
        #[case] doses: DosesShortSyntax,
        #[case] want: DosesShortSyntax,
    ) {
        init();

        let doses = from_short_syntax(doses);
        let want = from_short_syntax(want);

        let got = next_allowed_single(&doses, &limit);

        assert_eq!(got, Some(want));
    }
}

/*

limit: 3.5 every 5h

01:00 1        1
02:00 1        1 1
03:00 2        1 1 2
04:00 1        1 1 2 1         -> 5 -> OVER
05:00          1 1 2 1         -> 5 -> OVER
06:00            1 2 1         -> 4 -> OVER
07:00              2 1         -> 3 -> ALLOW 0.5
08:00                1         -> 1 -> ALLOW 2.5
09:00                          -> 0 -> ALLOW 3.5

01:00 1.0
02:00 1
03:00 2.0
04:00 1.2
05:00 2.0
06:00 1.3   (
07:00 1.4   (1+2.0+1.2+2.0+1.3+1.4)
08:00

*/
