// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

//! Cron schedules that follow the wall clock of a timezone.
//!
//! Every occurrence is computed from the real timezone rules, never from a UTC offset remembered
//! earlier, so reminders stay at the same local time across daylight saving changes (see
//! <https://github.com/lutzky/trufotbot/issues/30>).

use std::time::Duration;

use chrono::{DateTime, TimeDelta, TimeZone, Utc};
use croner::{
    Cron,
    errors::CronError,
    parser::{CronParser, Seconds},
};

/// The longest we sleep without re-reading the clock, so that wall clock jumps (NTP corrections,
/// suspend/resume) are noticed reasonably quickly.
const MAX_SLEEP: Duration = Duration::from_secs(30);

/// Occurrences that we only get to after this long past their scheduled time are skipped rather
/// than fired late, e.g. after the machine was suspended. Matches what happens if TrufotBot is
/// down at the scheduled time: the reminder is not sent when it comes back.
const MAX_LATENESS: TimeDelta = TimeDelta::minutes(5);

/// Parses a six-field cron schedule (seconds first), as used throughout TrufotBot.
pub fn parse(pattern: &str) -> Result<Cron, CronError> {
    CronParser::builder()
        .seconds(Seconds::Required)
        .dom_and_dow(true)
        // Accept `5/5`-style steps, which croner < 4 (and so older TrufotBot versions) allowed.
        .sloppy_ranges(true)
        .build()
        .parse(pattern)
}

/// The first instant strictly after `after` at which `cron` fires on the clock of `timezone`.
///
/// Fixed-time schedules that land in a daylight saving gap fire right after the gap, and ones that
/// land in an overlap fire only once; see croner's documentation.
pub fn next_fire<Tz: TimeZone>(
    cron: &Cron,
    timezone: &Tz,
    after: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    cron.find_next_occurrence(&after.with_timezone(timezone), false)
        .inspect_err(|e| {
            log::warn!(
                "Schedule {} has no occurrence after {after}: {e}",
                cron.pattern
            )
        })
        .ok()
        .map(|t| t.to_utc())
}

/// Calls `fire` with the scheduled time of every occurrence of `cron`, forever.
///
/// `clock` is read after every sleep. It is a parameter so that tests can run days of schedule
/// instantly, using a clock that follows tokio's paused time.
pub async fn run<Tz, C, F>(cron: Cron, timezone: Tz, clock: C, mut fire: F)
where
    Tz: TimeZone,
    C: Fn() -> DateTime<Utc>,
    F: FnMut(DateTime<Utc>),
{
    let mut after = clock();
    while let Some(next) = next_fire(&cron, &timezone, after) {
        while let Ok(remaining) = (next - clock()).to_std()
            && !remaining.is_zero()
        {
            tokio::time::sleep(remaining.min(MAX_SLEEP)).await;
        }

        let lateness = clock() - next;
        if lateness <= MAX_LATENESS {
            fire(next);
            // Continue from the exact instant we fired for, not from the clock; this is how
            // croner tells the two passes over a repeated hour apart.
            after = next;
        } else {
            log::warn!("Skipping schedule occurrence {next}, which is {lateness} in the past");
            after = clock();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use chrono::FixedOffset;
    use chrono_tz::{Australia::Lord_Howe, Europe::Dublin};
    use pretty_assertions::assert_eq;
    use tokio::time::Instant;

    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    /// A wall clock that starts at `start` and advances with tokio's (paused) time.
    fn clock_from(start: DateTime<Utc>) -> impl Fn() -> DateTime<Utc> {
        let started = Instant::now();
        move || start + TimeDelta::from_std(started.elapsed()).unwrap()
    }

    /// Scheduled times of `pattern` in the `days` days after `start`.
    async fn fires<Tz: TimeZone>(pattern: &str, tz: Tz, start: &str, days: i64) -> Vec<String> {
        let start = utc(start);
        let mut fired = vec![];
        tokio::select! {
            () = run(parse(pattern).unwrap(), tz, clock_from(start), |t| fired.push(t.to_rfc3339())) => {},
            () = tokio::time::sleep(Duration::from_secs(days as u64 * 24 * 3600)) => {},
        }
        fired
    }

    #[test]
    fn parse_requires_seconds() {
        assert!(parse("0 0 8 * * *").is_ok());
        assert!(parse("0 8 * * *").is_err());
        assert!(parse("not a schedule").is_err());
    }

    #[test]
    fn parse_accepts_sloppy_steps() {
        assert!(parse("0 5/5 * * * *").is_ok());
    }

    #[test]
    fn parse_matches_day_of_month_and_day_of_week() {
        // With dom_and_dow, this is "the 1st, if it's a Monday" rather than "the 1st or Mondays".
        let cron = parse("0 0 0 1 * MON").unwrap();
        let next = next_fire(&cron, &Utc, utc("2026-01-01T00:00:00Z")).unwrap();
        assert_eq!(next.to_rfc3339(), "2026-06-01T00:00:00+00:00");
    }

    #[test]
    fn next_fire_is_strictly_after() {
        let cron = parse("0 0 8 * * *").unwrap();
        let next = next_fire(&cron, &Utc, utc("2026-01-01T08:00:00Z")).unwrap();
        assert_eq!(next.to_rfc3339(), "2026-01-02T08:00:00+00:00");
    }

    #[tokio::test(start_paused = true)]
    async fn fall_back_keeps_local_time() {
        // Issue #30: Dublin leaves summer time on 2025-10-26.
        let got = fires("0 0 17 * * *", Dublin, "2025-10-25T15:59:55Z", 3).await;
        assert_eq!(
            got,
            [
                "2025-10-25T16:00:00+00:00",
                "2025-10-26T17:00:00+00:00",
                "2025-10-27T17:00:00+00:00"
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn offset_remembered_from_creation_reproduces_issue_30() {
        // Negative control: this is what tokio-cron-scheduler effectively did, using the UTC
        // offset at job creation (+01:00) forever. If this stops showing the bug, the other
        // tests here no longer prove anything.
        let summer = FixedOffset::east_opt(3600).unwrap();
        let got = fires("0 0 17 * * *", summer, "2025-10-25T15:59:55Z", 3).await;
        assert_eq!(
            got.get(1).map(String::as_str),
            Some("2025-10-26T16:00:00+00:00")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn spring_forward_keeps_local_time() {
        // Dublin enters summer time on 2026-03-29.
        let got = fires("0 0 17 * * *", Dublin, "2026-03-28T16:59:55Z", 3).await;
        assert_eq!(
            got,
            [
                "2026-03-28T17:00:00+00:00",
                "2026-03-29T16:00:00+00:00",
                "2026-03-30T16:00:00+00:00",
                "2026-03-31T16:00:00+00:00"
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fixed_time_in_spring_gap_fires_right_after_the_gap() {
        // 01:30 does not exist in Dublin on 2026-03-29; the clock jumps from 01:00 to 02:00.
        let got = fires("0 30 1 * * *", Dublin, "2026-03-28T00:00:00Z", 3).await;
        assert_eq!(
            got,
            [
                "2026-03-28T01:30:00+00:00",
                "2026-03-29T01:00:00+00:00",
                "2026-03-30T00:30:00+00:00"
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn fixed_time_in_fall_overlap_fires_once() {
        // 01:30 happens twice in Dublin on 2025-10-26.
        let got = fires("0 30 1 * * *", Dublin, "2025-10-25T12:00:00Z", 3).await;
        assert_eq!(
            got,
            [
                "2025-10-26T00:30:00+00:00",
                "2025-10-27T01:30:00+00:00",
                "2025-10-28T01:30:00+00:00"
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn wildcard_hours_fire_in_both_passes_of_the_overlap() {
        let got = fires("0 0 * * * *", Dublin, "2025-10-25T23:30:00Z", 1).await;
        assert_eq!(
            got.iter().take(4).map(String::as_str).collect::<Vec<_>>(),
            [
                "2025-10-26T00:00:00+00:00",
                "2025-10-26T01:00:00+00:00",
                "2025-10-26T02:00:00+00:00",
                "2025-10-26T03:00:00+00:00"
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn half_hour_dst_zone() {
        // Lord Howe's summer time is only 30 minutes ahead; it ends 2026-04-05 02:00 local.
        let got = fires("0 0 12 * * *", Lord_Howe, "2026-04-03T00:00:00Z", 4).await;
        assert_eq!(
            got,
            [
                "2026-04-03T01:00:00+00:00",
                "2026-04-04T01:00:00+00:00",
                "2026-04-05T01:30:00+00:00",
                "2026-04-06T01:30:00+00:00"
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn occurrences_long_past_after_a_clock_jump_are_skipped() {
        let start = utc("2026-01-01T00:00:00Z");
        let jump = Cell::new(TimeDelta::zero());
        let started = Instant::now();
        let clock = || start + TimeDelta::from_std(started.elapsed()).unwrap() + jump.get();
        let mut fired = vec![];
        {
            let schedule = run(parse("0 0 * * * *").unwrap(), Utc, clock, |t| {
                fired.push(t.to_rfc3339())
            });
            tokio::pin!(schedule);
            tokio::select! {
                () = &mut schedule => {},
                () = tokio::time::sleep(Duration::from_secs(10)) => {},
            }

            // Like a suspend/resume: 01:00 to 05:00 pass while nothing runs.
            jump.set(TimeDelta::hours(5) + TimeDelta::minutes(30));
            tokio::select! {
                () = &mut schedule => {},
                () = tokio::time::sleep(Duration::from_secs(3 * 3600)) => {},
            }
        }
        assert_eq!(
            fired,
            [
                "2026-01-01T06:00:00+00:00",
                "2026-01-01T07:00:00+00:00",
                "2026-01-01T08:00:00+00:00"
            ]
        );
    }
}
