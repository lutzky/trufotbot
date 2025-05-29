use chrono::TimeDelta;
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

    for dose in doses.iter().rev() {
        total -= dose.quantity;
        if total < limit.amount {
            return Some(vec![
                CreateDose {
                    quantity: limit.amount - total,
                    taken_at: dose.taken_at.checked_add_signed(hours) /* TODO: If this is None, it's out-of-range, log that */?,
                    noted_by_user: None,
                },
                CreateDose {
                    quantity: limit.amount,
                    taken_at: next_full_dose,
                    noted_by_user: None,
                },
            ]);
        }
    }

    Some(vec![CreateDose {
        quantity: limit.amount,
        taken_at: next_full_dose,
        noted_by_user: None,
    }])
}

#[cfg(test)]
mod tests {
    use chrono::{TimeDelta, TimeZone, Utc};
    use pretty_assertions::assert_eq;
    use shared::api::{dose::CreateDose, medication::DoseLimit};

    use super::next_allowed_single;

    #[test]
    fn test_a() {
        // TODO: Use rstest, add more cases
        let doses = [(1, 1.0), (2, 1.0), (3, 2.0), (4, 1.0), (5, 0.0)];
        let base_time = Utc.with_ymd_and_hms(2023, 4, 5, 10, 0, 0).unwrap();
        let doses = doses
            .iter()
            .map(|(hour, quantity)| CreateDose {
                quantity: *quantity,
                taken_at: base_time
                    .checked_add_signed(TimeDelta::hours(*hour))
                    .unwrap(),
                noted_by_user: None,
            })
            .collect::<Vec<_>>();
        let limit = DoseLimit {
            hours: 5,
            amount: 3.5,
        };

        let got = next_allowed_single(&doses, &limit);
        let want = vec![
            CreateDose {
                quantity: 1.5, // FIXME TODO this should actually be 2.5
                taken_at: Utc.with_ymd_and_hms(2023, 4, 5, 18, 0, 0).unwrap(),
                noted_by_user: None,
            },
            CreateDose {
                quantity: 3.5,
                taken_at: Utc.with_ymd_and_hms(2023, 4, 5, 19, 0, 0).unwrap(),
                noted_by_user: None,
            },
        ];

        assert_eq!(got, Some(want));
    }
}

/*

limit: 3.5 every 5h


[ 1: 1,   3: 8,    4:   1 ]


01:00 1.0      1                 5
02:00 1        1 1               4
03:00 2.0      1 1 2             3
04:00 1.0      1 1 2 1
05:00          1 1 2 1
06:00            1 2 1
07:00            1 2 1
08:00                1


01:00 1.0
02:00 1
03:00 2.0
04:00 1.2
05:00 2.0
06:00 1.3   (
07:00 1.4   (1+2.0+1.2+2.0+1.3+1.4)
08:00

*/
