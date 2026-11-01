// Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>
//
// SPDX-License-Identifier: GPL-3.0-only

use std::{collections::HashMap, sync::Arc};

use chrono::{DateTime, Utc};
use color_eyre::eyre;
use tokio::{sync::Mutex, task::JoinHandle};

use crate::{
    cron_schedule,
    ids::{MedicationId, PatientId},
    time::local_timezone,
};

type Callback = dyn Fn(PatientId, MedicationId) + Send + Sync;
type Clock = dyn Fn() -> DateTime<Utc> + Send + Sync;
type Tasks = HashMap<(PatientId, MedicationId), Vec<ScheduleTask>>;

/// Runs a task for every reminder schedule of every patient's medication, calling the callback
/// whenever one is due. Cloning is cheap; clones share the same schedules.
#[derive(Clone)]
pub struct ReminderScheduler {
    tasks: Arc<Mutex<Tasks>>,
    callback: Arc<Callback>,
    clock: Arc<Clock>,
}

/// Stops its task when dropped.
struct ScheduleTask(JoinHandle<()>);

impl Drop for ScheduleTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl ReminderScheduler {
    pub fn new<F>(callback: F) -> Self
    where
        F: Fn(PatientId, MedicationId) + Send + Sync + 'static,
    {
        // Not `crate::time::now`: schedule tasks are spawned, so they can't see `FAKE_TIME`.
        // Tests that care about time use `with_clock`.
        Self::with_clock(callback, Utc::now)
    }

    pub fn with_clock<F, C>(callback: F, clock: C) -> Self
    where
        F: Fn(PatientId, MedicationId) + Send + Sync + 'static,
        C: Fn() -> DateTime<Utc> + Send + Sync + 'static,
    {
        Self {
            tasks: Arc::default(),
            callback: Arc::new(callback),
            clock: Arc::new(clock),
        }
    }

    pub async fn remove_reminders<P, M>(&self, patient_id: P, medication_id: M)
    where
        P: Into<PatientId>,
        M: Into<MedicationId>,
    {
        self.tasks
            .lock()
            .await
            .remove(&(patient_id.into(), medication_id.into()));
    }

    pub async fn set_reminders_from_db(&self, db: &sqlx::SqlitePool) -> eyre::Result<()> {
        let rows = sqlx::query!(
            r#"
            SELECT
                patient_id,
                medication_id,
                cron_schedule
            FROM reminders
            "#,
        )
        .fetch_all(db)
        .await?;

        for row in rows {
            let patient_id = PatientId(row.patient_id);
            let medication_id = MedicationId(row.medication_id);
            let cron_schedules = row.cron_schedule.lines().collect::<Vec<_>>();

            if let Err(e) = self
                .set_reminders(patient_id, medication_id, &cron_schedules)
                .await
            {
                log::error!("Failed to set reminders for {patient_id:?}, {medication_id:?}: {e:?}");
            }
        }

        Ok(())
    }

    pub async fn remove_medication<M>(&self, medication_id: M)
    where
        M: Into<MedicationId>,
    {
        let medication_id: MedicationId = medication_id.into();
        self.tasks
            .lock()
            .await
            .retain(|(_, medication), _| *medication != medication_id);
    }

    pub async fn remove_patient<P>(&self, patient_id: P)
    where
        P: Into<PatientId>,
    {
        let patient_id: PatientId = patient_id.into();
        self.tasks
            .lock()
            .await
            .retain(|(patient, _), _| *patient != patient_id);
    }

    /// Replaces the reminders of this patient's medication. If any schedule is invalid, nothing
    /// changes.
    pub async fn set_reminders<P, M>(
        &self,
        patient_id: P,
        medication_id: M,
        cron_schedules: &[&str],
    ) -> eyre::Result<()>
    where
        P: Into<PatientId>,
        M: Into<MedicationId>,
    {
        let patient_id: PatientId = patient_id.into();
        let medication_id: MedicationId = medication_id.into();

        let schedules = cron_schedules
            .iter()
            .map(|schedule| {
                cron_schedule::parse(schedule)
                    .map(|cron| ((*schedule).to_owned(), cron))
                    .map_err(|e| eyre::eyre!("Invalid cron schedule {schedule:?}: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        log::info!(
            "Setting reminders for {patient_id:?} and {medication_id:?} to {cron_schedules:?}"
        );

        let new_tasks = schedules
            .into_iter()
            .map(|(schedule, cron)| {
                let callback = self.callback.clone();
                let clock = self.clock.clone();
                ScheduleTask(tokio::spawn(cron_schedule::run(
                    cron,
                    local_timezone(),
                    move || clock(),
                    move |_| {
                        log::debug!(
                            "Calling reminder callback for {patient_id:?}, {medication_id:?} \
                             on schedule {schedule:?}"
                        );
                        callback(patient_id, medication_id);
                    },
                )))
            })
            .collect();

        // Replacing the entry drops, and so stops, the previous tasks.
        self.tasks
            .lock()
            .await
            .insert((patient_id, medication_id), new_tasks);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::TimeDelta;
    use pretty_assertions::assert_eq;
    use pretty_env_logger::env_logger;
    use tokio::{sync::mpsc, time::Instant};

    use super::*;

    /// Never due, within the lifetime of a test.
    const YEARLY: &str = "0 0 0 1 1 *";

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    async fn initialize_scheduler() -> ReminderScheduler {
        init();
        #[allow(clippy::unreachable)]
        let scheduler = ReminderScheduler::new(|_, _| unreachable!());

        scheduler.set_reminders(1, 1, &[YEARLY]).await.unwrap();

        scheduler
    }

    async fn reminder_count<P, M>(
        scheduler: &ReminderScheduler,
        patient_id: P,
        medication_id: M,
    ) -> usize
    where
        P: Into<PatientId>,
        M: Into<MedicationId>,
    {
        scheduler
            .tasks
            .lock()
            .await
            .get(&(patient_id.into(), medication_id.into()))
            .map_or(0, Vec::len)
    }

    /// A scheduler whose clock follows tokio's paused time, starting at 2026-01-01T00:00:30Z,
    /// and a channel receiving (patient, medication) for every reminder due.
    fn scheduler_with_paused_clock() -> (
        ReminderScheduler,
        mpsc::UnboundedReceiver<(PatientId, MedicationId)>,
    ) {
        init();
        let start: DateTime<Utc> = "2026-01-01T00:00:30Z".parse().unwrap();
        let started = Instant::now();
        let (sender, receiver) = mpsc::unbounded_channel();
        let scheduler = ReminderScheduler::with_clock(
            move |patient, medication| {
                let _ = sender.send((patient, medication));
            },
            move || start + TimeDelta::from_std(started.elapsed()).unwrap(),
        );
        (scheduler, receiver)
    }

    fn received(
        receiver: &mut mpsc::UnboundedReceiver<(PatientId, MedicationId)>,
    ) -> Vec<(i64, i64)> {
        let mut out = vec![];
        while let Ok((patient, medication)) = receiver.try_recv() {
            out.push((patient.0, medication.0));
        }
        out
    }

    #[tokio::test]
    async fn test_replace() {
        let scheduler = initialize_scheduler().await;
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        let schedules = vec![YEARLY, "0 0 0 * * *"];
        scheduler.set_reminders(1, 1, &schedules).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 2);
    }

    #[tokio::test]
    async fn test_remove() {
        let scheduler = initialize_scheduler().await;
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        scheduler.remove_reminders(1, 1).await;
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 0);
    }

    #[tokio::test]
    async fn test_remove_medication() {
        let scheduler = initialize_scheduler().await;
        scheduler.set_reminders(1, 2, &[YEARLY]).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        assert_eq!(reminder_count(&scheduler, 1, 2).await, 1);
        scheduler.remove_medication(1).await;
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 0);
        assert_eq!(reminder_count(&scheduler, 1, 2).await, 1);
    }

    #[tokio::test]
    async fn test_remove_patient() {
        let scheduler = initialize_scheduler().await;
        scheduler.set_reminders(2, 1, &[YEARLY]).await.unwrap();
        scheduler.set_reminders(2, 2, &[YEARLY]).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        assert_eq!(reminder_count(&scheduler, 2, 1).await, 1);
        assert_eq!(reminder_count(&scheduler, 2, 2).await, 1);
        scheduler.remove_patient(2).await;
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        assert_eq!(reminder_count(&scheduler, 2, 1).await, 0);
        assert_eq!(reminder_count(&scheduler, 2, 2).await, 0);
    }

    #[tokio::test]
    async fn test_invalid_schedule_changes_nothing() {
        let scheduler = initialize_scheduler().await;
        let result = scheduler
            .set_reminders(1, 1, &["0 0 0 * * *", "not a schedule", "0 0 1 * * *"])
            .await;
        assert!(result.is_err());
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);

        // A 5-field schedule, which has no seconds, is invalid too.
        assert!(scheduler.set_reminders(1, 3, &["0 8 * * *"]).await.is_err());
        assert_eq!(reminder_count(&scheduler, 1, 3).await, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn test_callback_fires_when_due() {
        let (scheduler, mut receiver) = scheduler_with_paused_clock();
        scheduler
            .set_reminders(7, 8, &["0 * * * * *"])
            .await
            .unwrap();

        // Due at 00:01:00 and 00:02:00, but not yet 00:03:00.
        tokio::time::sleep(Duration::from_secs(150)).await;
        assert_eq!(received(&mut receiver), [(7, 8), (7, 8)]);
    }

    #[tokio::test(start_paused = true)]
    async fn test_each_schedule_fires_separately() {
        let (scheduler, mut receiver) = scheduler_with_paused_clock();
        scheduler
            .set_reminders(1, 1, &["0 * * * * *"])
            .await
            .unwrap();
        scheduler
            .set_reminders(2, 2, &["0 1 0 * * *"])
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_secs(90)).await;
        assert_eq!(received(&mut receiver), [(1, 1), (2, 2)]);
    }

    #[tokio::test(start_paused = true)]
    async fn test_replaced_and_removed_reminders_stop_firing() {
        let (scheduler, mut receiver) = scheduler_with_paused_clock();
        scheduler
            .set_reminders(1, 1, &["0 * * * * *"])
            .await
            .unwrap();
        scheduler
            .set_reminders(2, 2, &["0 * * * * *"])
            .await
            .unwrap();
        scheduler.set_reminders(1, 1, &[YEARLY]).await.unwrap();
        scheduler.remove_patient(2).await;

        tokio::time::sleep(Duration::from_secs(150)).await;
        assert_eq!(received(&mut receiver), []);
    }
}
