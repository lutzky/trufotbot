use std::collections::{HashMap, HashSet};

use tokio_cron_scheduler::JobSchedulerError;
use uuid::Uuid;

type PatientId = i64;
type MedicationId = i64;

pub struct ReminderScheduler {
    pub scheduler: tokio_cron_scheduler::JobScheduler,

    patient_jobs: HashMap<PatientId, HashMap<MedicationId, Vec<Uuid>>>,
    medication_patients: HashMap<MedicationId, HashSet<PatientId>>,
}

impl ReminderScheduler {
    pub async fn new() -> Result<Self, JobSchedulerError> {
        let scheduler = tokio_cron_scheduler::JobScheduler::new().await?;
        scheduler.start().await?;

        Ok(Self {
            scheduler,
            patient_jobs: Default::default(),
            medication_patients: Default::default(),
        })
    }

    pub async fn remove_reminders(
        &mut self,
        patient_id: PatientId,
        medication_id: MedicationId,
    ) -> Result<(), JobSchedulerError> {
        if let Some(jobs) = self
            .patient_jobs
            .get_mut(&patient_id)
            .and_then(|m| m.remove(&medication_id))
        {
            for job_id in jobs {
                self.scheduler.remove(&job_id).await?;
            }
        }

        if let Some(patients) = self.medication_patients.get_mut(&medication_id) {
            patients.remove(&patient_id);
            if patients.is_empty() {
                self.medication_patients.remove(&medication_id);
            }
        }

        Ok(())
    }

    // TODO: Add support for removing a patient, or removing a medication

    pub async fn set_reminders_from_db(&mut self, db: &sqlx::SqlitePool) -> anyhow::Result<()> {
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
            let patient_id = row.patient_id;
            let medication_id = row.medication_id;
            let cron_schedule = row.cron_schedule;

            self.set_reminders(patient_id, medication_id, &[cron_schedule])
                .await?;
        }

        Ok(())
    }

    pub async fn remove_medication(
        &mut self,
        medication_id: MedicationId,
    ) -> Result<(), JobSchedulerError> {
        if let Some(patients) = self.medication_patients.remove(&medication_id) {
            for patient_id in patients {
                self.remove_reminders(patient_id, medication_id).await?
            }
        }
        Ok(())
    }

    pub async fn set_reminders(
        &mut self,
        patient_id: PatientId,
        medication_id: MedicationId,
        cron_schedules: &[String],
    ) -> Result<(), JobSchedulerError> {
        self.remove_reminders(patient_id, medication_id).await?;

        log::info!(
            "Setting reminders for patient {patient_id} and medication {medication_id} to {cron_schedules:?}"
        );

        let jobs = cron_schedules
            .iter()
            .map(|schedule| {
                let schedule = schedule.clone();
                tokio_cron_scheduler::Job::new(schedule.clone(), move |_, _| {
                    // TODO: Actual reminder logic
                    // To accomplish that, we need to hold the "telegram sender"
                    // bits of AppState. Currently AppState holds a
                    // ReminderScheduler, so it can't hold it back (probably)...
                    // and we need to separate the "telegram sender" bits of
                    // AppState out (...effectively like we did with ReminderScheduler).
                    log::info!(
                        "This is a reminder for patient {} and medication {}: {:?}",
                        patient_id,
                        medication_id,
                        schedule
                    );
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let job_ids = {
            let mut job_ids = vec![];

            for job in jobs {
                let job_id = self.scheduler.add(job).await?;
                job_ids.push(job_id);
            }

            job_ids
        };

        let s = self
            .patient_jobs
            .entry(patient_id)
            .or_default()
            .entry(medication_id)
            .or_default();
        *s = job_ids;

        self.medication_patients
            .entry(medication_id)
            .or_default()
            .insert(patient_id);

        Ok(())
    }

    pub async fn start(&self) {
        self.scheduler.start().await.unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_env_logger::env_logger;

    fn init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    async fn initialize_scheduler() -> ReminderScheduler {
        init();
        let mut scheduler = ReminderScheduler::new().await.unwrap();

        scheduler
            .set_reminders(1, 1, &["* * * * * *".to_string()])
            .await
            .unwrap();

        scheduler
    }

    fn reminder_count(scheduler: &ReminderScheduler, patient_id: i64, medication_id: i64) -> usize {
        let Some(patient) = scheduler.patient_jobs.get(&patient_id) else {
            return 0;
        };
        let Some(medication) = patient.get(&medication_id) else {
            return 0;
        };
        medication.len()
    }

    #[tokio::test]
    async fn test_replace() {
        let mut scheduler = initialize_scheduler().await;
        assert_eq!(reminder_count(&scheduler, 1, 1), 1);
        let schedules = vec!["* * * * * *".to_string(), "0 0 0 * * *".to_string()];
        scheduler.set_reminders(1, 1, &schedules).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1), 2);
    }

    #[tokio::test]
    async fn test_remove() {
        let mut scheduler = initialize_scheduler().await;
        assert_eq!(reminder_count(&scheduler, 1, 1), 1);
        scheduler.remove_reminders(1, 1).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1), 0);
    }

    #[tokio::test]
    async fn test_remove_medication() {
        let mut scheduler = initialize_scheduler().await;
        scheduler
            .set_reminders(1, 2, &["* * * * * *".to_string()])
            .await
            .unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1), 1);
        assert_eq!(reminder_count(&scheduler, 1, 2), 1);
        scheduler.remove_medication(1).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1), 0);
        assert_eq!(reminder_count(&scheduler, 1, 2), 1);
    }
}
