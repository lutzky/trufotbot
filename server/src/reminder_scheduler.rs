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

    pub async fn set_reminders(
        &mut self,
        patient_id: PatientId,
        medication_id: MedicationId,
        cron_schedules: &[String],
    ) -> Result<(), JobSchedulerError> {
        self.remove_reminders(patient_id, medication_id).await?;

        let jobs = cron_schedules
            .iter()
            .map(|schedule| {
                let schedule = schedule.clone();
                tokio_cron_scheduler::Job::new(schedule.clone(), move |_, _| {
                    log::info!(
                        "Reminders for patient {} and medication {}: {:?}",
                        patient_id,
                        medication_id,
                        schedule
                    ); // TODO actual reminders please
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

    #[tokio::test]
    async fn test_reminder_scheduler() {
        let mut scheduler = ReminderScheduler::new().await.unwrap();

        let patient_id = 11;
        let medication_id = 22;
        let schedules = vec!["* * * * * *".to_string()];

        scheduler
            .set_reminders(patient_id, medication_id, &schedules)
            .await
            .unwrap();

        let schedules = vec!["* * * * * *".to_string(), "0 0 0 * * *".to_string()];

        scheduler
            .set_reminders(patient_id, medication_id, &schedules)
            .await
            .unwrap();

        assert_eq!(scheduler.patient_jobs.get(&patient_id).unwrap().len(), 1);
        assert_eq!(scheduler.medication_patients.len(), 1);

        scheduler
            .remove_reminders(patient_id, medication_id)
            .await
            .unwrap();

        assert_eq!(scheduler.patient_jobs.get(&patient_id).unwrap().len(), 0);
        assert!(
            scheduler
                .medication_patients
                .entry(medication_id)
                .or_default()
                .is_empty()
        );
    }
}
