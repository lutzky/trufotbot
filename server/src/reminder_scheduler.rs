use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use tokio::sync::Mutex;
use tokio_cron_scheduler::JobSchedulerError;
use uuid::Uuid;

// TODO: Consider using PatientId and MedicationId throughout the codebase. If
// you do that, have these functions only accept actual PatientId and
// MedicationId types, not Into<them>.

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub struct PatientId(pub i64);

impl From<i64> for PatientId {
    fn from(value: i64) -> Self {
        PatientId(value)
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub struct MedicationId(pub i64);

impl From<i64> for MedicationId {
    fn from(value: i64) -> Self {
        MedicationId(value)
    }
}

#[derive(Clone)]
pub struct ReminderScheduler {
    pub scheduler: tokio_cron_scheduler::JobScheduler,
    job_uuids: Arc<Mutex<JobUuids>>,

    callback: Arc<Box<dyn Fn(PatientId, MedicationId) + Send + Sync>>,
}

#[derive(Default)]
struct JobUuids {
    patient_jobs: HashMap<PatientId, HashMap<MedicationId, Vec<Uuid>>>,
    medication_patients: HashMap<MedicationId, HashSet<PatientId>>,
}

impl ReminderScheduler {
    pub async fn new<F>(callback: F) -> Result<Self, JobSchedulerError>
    where
        F: Fn(PatientId, MedicationId) + Send + Sync + 'static,
    {
        let scheduler = tokio_cron_scheduler::JobScheduler::new().await?;
        scheduler.start().await?;

        Ok(Self {
            scheduler,
            job_uuids: Arc::new(Mutex::new(Default::default())),
            callback: Arc::new(Box::new(callback)),
        })
    }

    pub async fn remove_reminders<P, M>(
        &mut self,
        patient_id: P,
        medication_id: M,
    ) -> Result<(), JobSchedulerError>
    where
        P: Into<PatientId>,
        M: Into<MedicationId>,
    {
        let patient_id: PatientId = patient_id.into();
        let medication_id: MedicationId = medication_id.into();

        let mut job_uuids = self.job_uuids.lock().await;

        if let Some(jobs) = job_uuids
            .patient_jobs
            .get_mut(&patient_id)
            .and_then(|m| m.remove(&medication_id))
        {
            for job_id in jobs {
                self.scheduler.remove(&job_id).await?;
            }
        }

        if let Some(patients) = job_uuids.medication_patients.get_mut(&medication_id) {
            patients.remove(&patient_id);
            if patients.is_empty() {
                job_uuids.medication_patients.remove(&medication_id);
            }
        }

        Ok(())
    }

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

    pub async fn remove_medication<M>(&mut self, medication_id: M) -> Result<(), JobSchedulerError>
    where
        M: Into<MedicationId>,
    {
        let medication_id: MedicationId = medication_id.into();

        let patients = self
            .job_uuids
            .lock()
            .await
            .medication_patients
            .remove(&medication_id);

        if let Some(patients) = patients {
            for patient_id in patients {
                self.remove_reminders(patient_id, medication_id).await?
            }
        }
        Ok(())
    }

    pub async fn remove_patient<P>(&mut self, patient_id: P) -> Result<(), JobSchedulerError>
    where
        P: Into<PatientId>,
    {
        let patient_id: PatientId = patient_id.into();

        let mut job_uuids = self.job_uuids.lock().await;

        let Some(medications) = job_uuids.patient_jobs.remove(&patient_id) else {
            return Ok(());
        };

        for (medication_id, jobs) in medications {
            for job_id in jobs {
                self.scheduler.remove(&job_id).await?;
            }

            if let Some(patients) = job_uuids.medication_patients.get_mut(&medication_id) {
                patients.remove(&patient_id);
                if patients.is_empty() {
                    job_uuids.medication_patients.remove(&medication_id);
                }
            }
        }

        Ok(())
    }

    pub async fn set_reminders<P, M>(
        &mut self,
        patient_id: P,
        medication_id: M,
        cron_schedules: &[&str],
    ) -> Result<(), JobSchedulerError>
    where
        P: Into<PatientId>,
        M: Into<MedicationId>,
    {
        let patient_id: PatientId = patient_id.into();
        let medication_id: MedicationId = medication_id.into();

        self.remove_reminders(patient_id, medication_id).await?;

        log::info!(
            "Setting reminders for {patient_id:?} and {medication_id:?} to {cron_schedules:?}"
        );

        let jobs = cron_schedules
            .iter()
            .map(|schedule| {
                let schedule: String = (*schedule).into();
                let callback = self.callback.clone();
                tokio_cron_scheduler::Job::new(schedule.clone(), move |_, _| {
                    // TODO: Actual reminder logic
                    // To accomplish that, we need to hold the "telegram sender"
                    // bits of AppState. Currently AppState holds a
                    // ReminderScheduler, so it can't hold it back (probably)...
                    // and we need to separate the "telegram sender" bits of
                    // AppState out (...effectively like we did with ReminderScheduler).
                    log::info!(
                        "This is a reminder for {patient_id:?} and {medication_id:?}: {schedule:?}",
                    );
                    callback(patient_id, medication_id);
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

        let mut job_uuids = self.job_uuids.lock().await;

        let s = job_uuids
            .patient_jobs
            .entry(patient_id)
            .or_default()
            .entry(medication_id)
            .or_default();
        *s = job_ids;

        job_uuids
            .medication_patients
            .entry(medication_id)
            .or_default()
            .insert(patient_id);

        Ok(())
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
        let mut scheduler = ReminderScheduler::new(|_, _| unreachable!()).await.unwrap();

        scheduler
            .set_reminders(1, 1, &["* * * * * *"])
            .await
            .unwrap();

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
        let patient_id: PatientId = patient_id.into();
        let medication_id: MedicationId = medication_id.into();
        let job_uuids = scheduler.job_uuids.lock().await;

        let Some(patient) = job_uuids.patient_jobs.get(&patient_id) else {
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
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        let schedules = vec!["* * * * * *", "0 0 0 * * *"];
        scheduler.set_reminders(1, 1, &schedules).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 2);
    }

    #[tokio::test]
    async fn test_remove() {
        let mut scheduler = initialize_scheduler().await;
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        scheduler.remove_reminders(1, 1).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 0);
    }

    #[tokio::test]
    async fn test_remove_medication() {
        let mut scheduler = initialize_scheduler().await;
        scheduler
            .set_reminders(1, 2, &["* * * * * *"])
            .await
            .unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        assert_eq!(reminder_count(&scheduler, 1, 2).await, 1);
        scheduler.remove_medication(1).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 0);
        assert_eq!(reminder_count(&scheduler, 1, 2).await, 1);
    }

    #[tokio::test]
    async fn test_remove_patient() {
        let mut scheduler = initialize_scheduler().await;
        scheduler
            .set_reminders(2, 1, &["* * * * * *"])
            .await
            .unwrap();
        scheduler
            .set_reminders(2, 2, &["* * * * * *"])
            .await
            .unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        assert_eq!(reminder_count(&scheduler, 2, 1).await, 1);
        assert_eq!(reminder_count(&scheduler, 2, 2).await, 1);
        scheduler.remove_patient(2).await.unwrap();
        assert_eq!(reminder_count(&scheduler, 1, 1).await, 1);
        assert_eq!(reminder_count(&scheduler, 2, 1).await, 0);
        assert_eq!(reminder_count(&scheduler, 2, 2).await, 0);
    }
}
