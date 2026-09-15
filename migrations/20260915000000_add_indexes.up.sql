CREATE INDEX idx_doses_patient_medication_taken_at
    ON doses (patient_id, medication_id, taken_at);

CREATE INDEX idx_doses_medication_id
    ON doses (medication_id);

CREATE INDEX idx_reminders_medication_id
    ON reminders (medication_id);
