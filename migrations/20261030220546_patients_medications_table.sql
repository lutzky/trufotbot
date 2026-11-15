ALTER TABLE reminders RENAME TO patients_medications;
ALTER TABLE patients_medications RENAME COLUMN cron_schedule TO reminder_cron_schedule;

DROP INDEX idx_reminders_medication_id;
CREATE INDEX idx_patients_medications_medication_id ON patients_medications (medication_id);
