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

    dbg!(&last_non_zero, &epoch_start, &next_full_dose, total);

    for dose in doses.iter() {
        total -= dose.quantity;
        println!("After decreasing {dose:?}, total is {total}");
        if total < limit.amount {
            println!("...which is under the limit!");
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

    fn from_hm(hm: &str) -> TimeDelta {
        let (h, m) = hm.split_once(":").unwrap();
        TimeDelta::minutes(60 * h.parse::<i64>().unwrap() + m.parse::<i64>().unwrap())
    }

    #[test]
    fn test_a() {
        // TODO: Use rstest, add more cases
        let doses = [
            ("1:00", 1.0),
            ("2:00", 1.0),
            ("3:00", 2.0),
            ("4:00", 1.0),
            ("5:00", 0.0),
        ];
        let base_time = Utc.with_ymd_and_hms(2023, 4, 5, 0, 0, 0).unwrap();
        let doses = doses
            .iter()
            .map(|(when, quantity)| CreateDose {
                quantity: *quantity,
                taken_at: base_time.checked_add_signed(from_hm(when)).unwrap(),
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
                quantity: 0.5,
                taken_at: base_time.checked_add_signed(from_hm("07:00")).unwrap(),
                noted_by_user: None,
            },
            CreateDose {
                quantity: 3.5,
                taken_at: base_time.checked_add_signed(from_hm("09:00")).unwrap(),
                noted_by_user: None,
            },
        ];

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
