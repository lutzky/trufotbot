ALTER TABLE patients_medications ADD dose_limits TEXT NOT NULL DEFAULT '';

UPDATE patients_medications
SET
    dose_limits = (
        SELECT m.dose_limits
        FROM medications AS m
        WHERE m.id = patients_medications.medication_id
    )
WHERE EXISTS (
    SELECT 1
    FROM medications AS m
    WHERE
        m.id = patients_medications.medication_id
        AND m.dose_limits IS NOT NULL
        AND m.dose_limits <> ''
);

INSERT INTO patients_medications (
    patient_id,
    medication_id,
    reminder_cron_schedule,
    dose_limits
)
SELECT DISTINCT
    d.patient_id,
    m.id AS medication_id,
    '' AS reminder_cron_schedule,
    m.dose_limits
FROM medications AS m
    INNER JOIN doses AS d ON m.id = d.medication_id
WHERE
    m.dose_limits IS NOT NULL
    AND m.dose_limits <> ''
ON CONFLICT (patient_id, medication_id) DO UPDATE
SET dose_limits = excluded.dose_limits;

INSERT INTO patients_medications (
    patient_id,
    medication_id,
    reminder_cron_schedule,
    dose_limits
)
SELECT
    p.id AS patient_id,
    m.id AS medication_id,
    '' AS reminder_cron_schedule,
    m.dose_limits
FROM medications AS m
    CROSS JOIN patients AS p
WHERE
    m.dose_limits IS NOT NULL
    AND m.dose_limits <> ''
    AND NOT EXISTS (
        SELECT 1
        FROM patients_medications AS pm
        WHERE pm.medication_id = m.id
    );

ALTER TABLE medications DROP COLUMN dose_limits;
